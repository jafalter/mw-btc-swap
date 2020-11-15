use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct JsonRpc {
    jsonrpc : String,
    id : String,
    method : String,
    params : Vec<String>
}

pub struct JsonRPCHttpRequest {
    url : String,
    method : String,
    body : JsonRpc
}

pub struct HttpResponse {
    status : u16,
    content : String
}

impl JsonRpc {
    pub fn new(jsonrpc : String, id : String, method : String, params : Vec<String>) -> JsonRpc {
        JsonRpc {
            jsonrpc : jsonrpc,
            id : id,
            method : method,
            params : params
        }
    }
}

impl JsonRPCHttpRequest {
    pub fn newJsonRpc(url : String, method : String, body : JsonRpc) -> JsonRPCHttpRequest {
        JsonRPCHttpRequest {
            url : url,
            method : method,
            body : body
        }
    }

    pub async fn execute(&self) -> Result<HttpResponse,&'static str> {
        let client = reqwest::Client::new();
        let res = client.post(&self.url)
                .json(&self.body)
                .send()
                .await
                .expect("Http Request failed");
        let status = res.status();
        if status.is_success() {
            let content = res.text()
                .await
                .expect("Failed to read http response");
            Ok(HttpResponse {
                status : status.as_u16(),
                content : content
            })
        }
        else {
            Err("Failed with invalid respcode")
        }
    }
}