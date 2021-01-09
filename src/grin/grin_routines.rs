use grin_util::secp::pedersen::Commitment;
use grin_util::secp::SecretKey;
use grin_util::secp::{
    key::ZERO_KEY,
    pedersen::{ProofMessage, RangeProof},
    ContextFlag, PublicKey, Secp256k1,
};
use rand::rngs::OsRng;

use crate::{constants::NANO_GRIN, util::get_os_rng};

pub struct MPBPContext {
    t_1: PublicKey,
    t_2: PublicKey,
    amount: u64,
    shared_nonce: SecretKey,
    tau_x: SecretKey,
    rng: OsRng,
    secp: Secp256k1,
}

impl MPBPContext {
    fn new(shared_nonce: SecretKey, amount: u64) -> MPBPContext {
        let rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        MPBPContext {
            t_1: PublicKey::new(),
            t_2: PublicKey::new(),
            amount: amount,
            shared_nonce: shared_nonce,
            tau_x: ZERO_KEY,
            rng: rng,
            secp: secp,
        }
    }

    fn add_t_1(&mut self, t_1: &PublicKey) {
        self.t_1 = PublicKey::from_combination(&self.secp, vec![&self.t_1.clone(), t_1]).unwrap();
    }

    fn add_t_2(&mut self, t_2: &PublicKey) {
        self.t_2 = PublicKey::from_combination(&self.secp, vec![&self.t_2.clone(), t_2]).unwrap();
    }

    fn add_tau_x(&mut self, tau_x: SecretKey) {
        self.tau_x.add_assign(&self.secp, &tau_x).unwrap();
    }
}

/// Randomly generate a new secrety key which can be used in commitments and proofs
///
/// # Arguments
/// * `rng` randomness generator
/// * `secp` Elliptic curve functionalities
pub fn create_secret_key(rng: &mut OsRng, secp: &Secp256k1) -> SecretKey {
    SecretKey::new(secp, rng)
}

/// Serialze a secret key to a hex encoded string
///
/// # Arguments
/// * `key` the key to serialize
pub fn serialize_secret_key(key: &SecretKey) -> String {
    hex::encode(&key)
}

/// Deserialize a hexdecoded string to a SecretKey object
///
/// # Arguments
/// * `key` the key serialized as hex string
/// * `secp` Secp256 functionatlity
pub fn deserialize_secret_key(key: &String, secp: &Secp256k1) -> SecretKey {
    SecretKey::from_slice(
        secp,
        &hex::decode(key).expect("Failed to deserialize a secret key from hex string"),
    )
    .expect("Failed to deserialize a secret key from hex string")
}

/// Serialize a pedersen commitment to a hex encoded string
///
/// # Arguments
/// * `com` pedersen commitment instance
pub fn serialize_commitment(com: &Commitment) -> String {
    hex::encode(&com)
}

/// Deserialize a pedersen commitment from a hex encoded string
///
/// # Arguments
///
/// * `com` commitment encoded as hex string
pub fn deserialize_commitment(com: &String) -> Commitment {
    Commitment::from_vec(hex::decode(com).expect("Failed to deserialize pedersen commitment"))
}

/// Conversion from grin to nanogrin
///
/// # Arguments
///
/// * `value` value in grins
pub fn grin_to_nanogrin(value: u64) -> u64 {
    value * NANO_GRIN
}

/// Round 1 of the multiparty bulletproof creation
/// In this round the parties do not yet the final coin commitment, however
/// they already know their part of the blinding factor and amount which will be commited to.
/// This is enough to create their part of T1 and T2 values, which we can then later sum up
/// with the second party.
/// The function returns an updated proof context containing their part of the T1 and T2
///
/// # Arguments
///
/// * `ctx` The initial context object of the proof
/// * `blind` The share of the commitment blinding factor
/// * `sec_nonce` A secret nonce used for proof creation
pub fn mp_bullet_proof_r1(
    mut ctx: MPBPContext,
    blind: SecretKey,
    sec_nonce: &SecretKey,
    commit: Commitment,
) -> Result<MPBPContext, String> {
    // These will be filled during the round
    let mut t_1 = PublicKey::new();
    let mut t_2 = PublicKey::new();
    let message: Option<ProofMessage> = None;
    let extra_data: Option<Vec<u8>> = None;
    let mut commits = vec![];
    commits.push(commit);

    println!("amount: {}", ctx.amount);
    println!("blind: {}", serialize_secret_key(&blind.clone()));
    println!(
        "shared_nonce: {}",
        serialize_secret_key(&ctx.shared_nonce.clone())
    );
    println!("commit: {}", serialize_commitment(&commit));
    println!("sec_nonce: {}", serialize_secret_key(&sec_nonce));

    ctx.secp.bullet_proof_multisig(
        ctx.amount,
        blind.clone(),
        ctx.shared_nonce.clone(),
        extra_data,
        message,
        None,
        Some(&mut t_1),
        Some(&mut t_2),
        commits.clone(),
        Some(sec_nonce),
        1,
    );
    ctx.add_t_1(&t_1);
    ctx.add_t_2(&t_2);
    Ok(ctx)
}

/// Round 2 of the multiparty bulletproof creation
/// In this round the parties know the sum of the T1 and T2 and the final commitment, they
/// are now computing their parts of the tau_x which then again gets summed together with the other party
/// The function returns an updated proof context with their part of the tau_x added
///
/// # Arguments
///
/// * `ctx` the proof context already filled with the T1 and T2 values
/// * `blind` the parties share of the blinding factor
/// * `commit` the final commitment
/// * `sec_nonce` the parties secret nonce
pub fn mp_bullet_proof_r2(
    mut ctx: MPBPContext,
    blind: &SecretKey,
    commit: Commitment,
    sec_nonce: SecretKey,
) -> Result<MPBPContext, String> {
    let mut tau_x = create_secret_key(&mut ctx.rng, &ctx.secp);
    ctx.secp.bullet_proof_multisig(
        ctx.amount,
        blind.clone(),
        ctx.shared_nonce.clone(),
        None,
        None,
        Some(&mut tau_x),
        Some(&mut ctx.t_1.clone()),
        Some(&mut ctx.t_2.clone()),
        vec![commit],
        Some(&sec_nonce),
        2,
    );
    ctx.add_tau_x(tau_x.clone());
    Ok(ctx)
}

/// The final proof creation algorithm callable be either one of the two parties to creat the final range proof
///
/// # Arguments
///
/// * `ctx` the proof context with the T1 T2 sum as well as the tau_x sum filled
/// * `blind` the parties share of the blinding factor
/// * `commit` the final commitment
/// * `sec_nonce` the parties secret nonce
pub fn mp_bullet_proof_fin(
    mut ctx: MPBPContext,
    blind: &SecretKey,
    commit: Commitment,
    sec_nonce: SecretKey,
) -> Result<RangeProof, String> {
    let proof = ctx
        .secp
        .bullet_proof_multisig(
            ctx.amount,
            blind.clone(),
            ctx.shared_nonce,
            None,
            None,
            Some(&mut ctx.tau_x),
            Some(&mut ctx.t_1),
            Some(&mut ctx.t_2),
            vec![commit],
            Some(&sec_nonce),
            0,
        )
        .expect("Failed to finalize MP bulletproof");
    Ok(proof)
}

#[cfg(test)]
mod test {
    use grin_util::secp::{ContextFlag, PublicKey, Secp256k1, SecretKey, pedersen::{Commitment, ProofMessage, ProofRange, RangeProof}};
    use grin_util::secp::Error;

    use crate::util::get_os_rng;

    use super::{
        create_secret_key, deserialize_commitment, deserialize_secret_key, grin_to_nanogrin,
        mp_bullet_proof_fin, mp_bullet_proof_r1, mp_bullet_proof_r2, serialize_commitment,
        serialize_secret_key, MPBPContext,
    };

    #[test]
    fn test_key_serializiation() {
        let mut rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let sk = create_secret_key(&mut rng, &secp);
        let ser = serialize_secret_key(&sk);
        let deser = deserialize_secret_key(&ser, &secp);
        assert_eq!(sk, deser);
    }

    #[test]
    fn test_commit_serialization() {
        let mut rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let sk = create_secret_key(&mut rng, &secp);
        let value: u64 = 1000000;
        let commit = secp.commit(value, sk).unwrap();
        let ser = serialize_commitment(&commit);
        let deser = deserialize_commitment(&ser);
        assert_eq!(commit, deser);
    }

    #[test]
    fn test_mp_bulletproof() {
        // Setup
        let mut rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let bf_a = create_secret_key(&mut rng, &secp);
        let bf_b = create_secret_key(&mut rng, &secp);
        let sec_nonce_a = create_secret_key(&mut rng, &secp);
        let sec_nonce_b = create_secret_key(&mut rng, &secp);
        let amount = grin_to_nanogrin(2);
        let shared_nonce = create_secret_key(&mut rng, &secp);
        let ctx = MPBPContext::new(shared_nonce, amount);
        let commit_a = secp.commit(amount, bf_a.clone()).unwrap();
        let commit_b = secp.commit(0, bf_b.clone()).unwrap();
        let commit = secp.commit_sum(vec![commit_a, commit_b], vec![]).unwrap();

        println!("Starting round 1");
        // Round 1
        let ctx = mp_bullet_proof_r1(ctx, bf_a.clone(), &sec_nonce_a, commit.clone()).unwrap();
        println!("Completed round 1 A");
        let ctx = mp_bullet_proof_r1(ctx, bf_b.clone(), &sec_nonce_b, commit.clone()).unwrap();
        println!("Completed round 1 B");

        // Round 2
        println!("Starting round 2");
        let ctx = mp_bullet_proof_r2(ctx, &bf_a, commit.clone(), sec_nonce_a.clone()).unwrap();
        println!("Completed round 2 A");
        let ctx = mp_bullet_proof_r2(ctx, &bf_b, commit.clone(), sec_nonce_b.clone()).unwrap();
        println!("Completed round 2 B");

        println!("Finalizing proof");
        let proof = mp_bullet_proof_fin(ctx, &bf_a, commit.clone(), sec_nonce_a.clone()).unwrap();

        assert!(secp
            .verify_bullet_proof(commit.clone(), proof.clone(), None)
            .is_ok());
    }

    #[test]
    fn test_bullet_proof_multisig() {
        let multisig_bp = |v,
                           nonce: SecretKey,
                           ca,
                           cb,
                           ba,
                           bb,
                           msg,
                           extra|
         -> (RangeProof, Result<ProofRange, Error>) {
            let secp = Secp256k1::with_caps(ContextFlag::Commit);
            let blinding_a: SecretKey = ba;
            let value: u64 = v;
            let partial_commit_a: Commitment = ca;

            let blinding_b: SecretKey = bb;
            let partial_commit_b: Commitment = cb;

            let message: Option<ProofMessage> = msg;
            let extra_data: Option<Vec<u8>> = extra;

            // upfront step: party A and party B generate self commitment and communicate to each other,
            //   to get the total commitment.
            let commit = secp
                .commit_sum(vec![partial_commit_a, partial_commit_b], vec![])
                .unwrap();
            let mut commits = vec![];
            commits.push(commit);

            let common_nonce = nonce;

            let private_nonce_a = SecretKey::new(&secp, &mut get_os_rng());
            let private_nonce_b = SecretKey::new(&secp, &mut get_os_rng());

            // 1st step on party A: generate t_one and t_two, and sends to party B
            let mut t_one_a = PublicKey::new();
            let mut t_two_a = PublicKey::new();
            secp.bullet_proof_multisig(
                value,
                blinding_a.clone(),
                common_nonce.clone(),
                extra_data.clone(),
                message.clone(),
                None,
                Some(&mut t_one_a),
                Some(&mut t_two_a),
                commits.clone(),
                Some(&private_nonce_a),
                1,
            );

            // 1st step on party B: generate t_one and t_two, and sends to party A
            let mut t_one_b = PublicKey::new();
            let mut t_two_b = PublicKey::new();
            secp.bullet_proof_multisig(
                value,
                blinding_b.clone(),
                common_nonce.clone(),
                extra_data.clone(),
                message.clone(),
                None,
                Some(&mut t_one_b),
                Some(&mut t_two_b),
                commits.clone(),
                Some(&private_nonce_b),
                1,
            );

            // 1st step on both party A and party B: sum up both t_one and both t_two.
            let mut pubkeys = vec![];
            pubkeys.push(&t_one_a);
            pubkeys.push(&t_one_b);
            let mut t_one_sum = PublicKey::from_combination(&secp, pubkeys.clone()).unwrap();

            pubkeys.clear();
            pubkeys.push(&t_two_a);
            pubkeys.push(&t_two_b);
            let mut t_two_sum = PublicKey::from_combination(&secp, pubkeys.clone()).unwrap();

            // 2nd step on party A: use t_one_sum and t_two_sum to generate tau_x, and sent to party B.
            let mut tau_x_a = SecretKey::new(&secp, &mut get_os_rng());
            secp.bullet_proof_multisig(
                value,
                blinding_a.clone(),
                common_nonce.clone(),
                extra_data.clone(),
                message.clone(),
                Some(&mut tau_x_a),
                Some(&mut t_one_sum),
                Some(&mut t_two_sum),
                commits.clone(),
                Some(&private_nonce_a),
                2,
            );

            // 2nd step on party B: use t_one_sum and t_two_sum to generate tau_x, and send to party A.
            let mut tau_x_b = SecretKey::new(&secp, &mut get_os_rng());
            secp.bullet_proof_multisig(
                value,
                blinding_b.clone(),
                common_nonce.clone(),
                extra_data.clone(),
                message.clone(),
                Some(&mut tau_x_b),
                Some(&mut t_one_sum),
                Some(&mut t_two_sum),
                commits.clone(),
                Some(&private_nonce_b),
                2,
            );

            // 2nd step on both party A and B: sum up both tau_x
            let mut tau_x_sum = tau_x_a;
            tau_x_sum.add_assign(&secp, &tau_x_b).unwrap();

            // 3rd step: party A finalizes bulletproof with input tau_x, t_one, t_two.
            let bullet_proof = secp
                .bullet_proof_multisig(
                    value,
                    blinding_a.clone(),
                    common_nonce.clone(),
                    extra_data.clone(),
                    message.clone(),
                    Some(&mut tau_x_sum),
                    Some(&mut t_one_sum),
                    Some(&mut t_two_sum),
                    commits.clone(),
                    Some(&private_nonce_a),
                    0,
                )
                .unwrap();

            // correct verification
            println!("MultiSig Bullet proof len: {:}", bullet_proof.len());
            let proof_range = secp.verify_bullet_proof(commit, bullet_proof, None);

            return (bullet_proof, proof_range);
        };

        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let value: u64 = 12345678;

        let common_nonce = SecretKey::new(&secp, &mut get_os_rng());

        let blinding_a = SecretKey::new(&secp, &mut get_os_rng());
        let partial_commit_a = secp.commit(value, blinding_a.clone()).unwrap();

        let blinding_b = SecretKey::new(&secp, &mut get_os_rng());
        let partial_commit_b = secp.commit(0, blinding_b.clone()).unwrap();

        // 1. Test Bulletproofs multisig without message
        let (_, proof_range) = multisig_bp(
            value,
            common_nonce.clone(),
            partial_commit_a,
            partial_commit_b,
            blinding_a.clone(),
            blinding_b.clone(),
            None,
            None,
        );
        assert_eq!(proof_range.unwrap().min, 0);

        // 2. wrong value committed to
        let wrong_partial_commit_a = secp.commit(87654321, blinding_a.clone()).unwrap();
        let (_, proof_range) = multisig_bp(
            value,
            common_nonce.clone(),
            wrong_partial_commit_a,
            partial_commit_b,
            blinding_a.clone(),
            blinding_b.clone(),
            None,
            None,
        );
        if !proof_range.is_err() {
            panic!("Multi-Sig Bullet proof verify should have error");
        }

        // 3. wrong blinding
        let wrong_blinding = SecretKey::new(&secp, &mut get_os_rng());
        let (_, proof_range) = multisig_bp(
            value,
            common_nonce.clone(),
            partial_commit_a,
            partial_commit_b,
            wrong_blinding,
            blinding_b.clone(),
            None,
            None,
        );
        if !proof_range.is_err() {
            panic!("Multi-Sig Bullet proof verify should have error");
        }

        // 4. Commit to a message in the bulletproof
        let message_bytes: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let message = ProofMessage::from_bytes(&message_bytes);
        let (_, proof_range) = multisig_bp(
            value,
            common_nonce,
            partial_commit_a,
            partial_commit_b,
            blinding_a,
            blinding_b,
            Some(message.clone()),
            None,
        );
        assert_eq!(proof_range.unwrap().min, 0);
    }
}
