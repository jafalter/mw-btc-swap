use crate::enums::HttpMethod;
use serde::{Deserialize, Serialize, Serializer, ser::SerializeSeq};
use std::clone::Clone;
use std::time::Duration;

#[derive(Deserialize)]
pub enum JsonRpcParam {
    String(String),
    Int(u64),
    Bool(bool),
    Vec(Vec<String>)
}

impl Serialize for JsonRpcParam {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        match self {
            JsonRpcParam::String(x) => serializer.serialize_str(&x),
            JsonRpcParam::Int(x) => serializer.serialize_u64(*x),
            JsonRpcParam::Bool(x) => serializer.serialize_bool(*x),
            JsonRpcParam::Vec(x) => {
                let mut seq = serializer.serialize_seq(Some(x.len()))?;
                for e in x {
                    seq.serialize_element(&e)?
                }
                seq.end()
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpc {
    jsonrpc : String,
    id : String,
    method : String,
    params : Vec<JsonRpcParam>
}

pub struct BasicAuth {
    username : String,
    password : Option<String>
}


pub struct RequestFactory {
    response_stub : Option<HttpResponse>
}


pub struct HttpRequest {
    url : String,
    method : HttpMethod,
    body : JsonRpc,
    auth : Option<BasicAuth>,
    response_stub : Option<HttpResponse>
}

pub struct HttpResponse {
    pub status : u16,
    pub content : String
}

impl Clone for HttpResponse {
    fn clone(&self) -> HttpResponse {
        HttpResponse {
            status: self.status,
            content: self.content.clone()
        }
    }
}

impl JsonRpc {

    /// Constructa new Json Request which can be sent via HTTP
    /// 
    /// # Arguments 
    /// * `jsonrpc` the jsonrpc version
    /// * `method` the remote method which should be called
    /// * `params` list of parameters passed to the method, must be a json encoded string 
    pub fn new(jsonrpc : String, id : String, method : String, params : Vec<JsonRpcParam>) -> JsonRpc {
        JsonRpc {
            jsonrpc : jsonrpc,
            id : id,
            method : method,
            params : params
        }
    }
}

impl RequestFactory {
    /// Construct a new RequestFactory
    /// 
    /// # Arguments
    /// * `response_stub` If passed then no actual http request will be made 
    ///                   instead the passed response will be returned which
    ///                   can be used in testing
    pub fn new(response_stub : Option<HttpResponse>) -> RequestFactory {
        RequestFactory{
            response_stub
        }
    }

    /// Construct a new JSON RPC Http Request
    /// 
    /// # Arguments
    /// * `self` reference to called object
    /// * `url` the request URL
    /// * `method` the http method (GET, POST)
    /// * `body` the http body which is the JsonRPCrequest
    pub fn new_json_rpc_request(&self, url : String, body : JsonRpc, username : String, password : String) -> HttpRequest {
        let auth = BasicAuth {
            username : username,
            password : Some(password)
        };
        HttpRequest {
            url : url,
            method : HttpMethod::POST,
            body : body,
            auth : Some(auth),
            response_stub : self.response_stub.clone()
        }
    }
}

impl HttpRequest {

    /// Execute a http request and return its result
    pub fn execute(&self) -> Result<HttpResponse,String> {
        if self.response_stub.is_some() {
            let r = self.response_stub.as_ref().unwrap();
            if r.status != 200 {
                Err(format!("RPC Request failed with invalid respcode: {}", r.status))
            }
            else {
                Ok(HttpResponse {
                    status : r.status,
                    content : r.content.clone()
                })
            }
        }
        else {
            let client = reqwest::blocking::Client::new();
        if self.method == HttpMethod::POST {
            println!("Sending request to URL: {}", self.url);
            let body = serde_json::to_string(&self.body).unwrap();
            println!("Sending post body: {}", body);
            let mut req = client.post(&self.url)
                .timeout(Duration::new(30, 0))
                .json(&self.body);
            match &self.auth {
                Some(x) => req = req.basic_auth(&x.username, x.password.as_ref()),
                _ => ()
            } 
            let res = req.send()
                .expect("Http Request failed");
            let status = res.status();
            let content = res.text()
                    .expect("Failed to read http response");
            if status.is_success() {
                Ok(HttpResponse {
                    status : status.as_u16(),
                    content : content
                })
            }
            else {
                Err(format!("RPC Request failed with invalid respcode: {} {}", status.as_str(), content))
            }   
        }
        else {
            Err(String::from("Http method currently not implemented"))
        }
        }
    }

}

#[cfg(test)]
mod test {
    use super::JsonRpcParam;

    #[test]
    fn test_rpc_param_str_serialization() {
        let mut params : Vec<JsonRpcParam> = Vec::new();
        params.push(JsonRpcParam::String("Test".to_string()));
        params.push(JsonRpcParam::Bool(true));
        params.push(JsonRpcParam::Int(64));
        params.push(JsonRpcParam::Vec(vec![String::from("abc"), String::from("cde")]));
        let serialized = serde_json::to_string(&params).unwrap();
        println!("{}",serialized);
        assert_eq!(r#"["Test",true,64,["abc","cde"]]"#, serialized);
    }
}
