use crate::enums::HttpMethod;
use serde::{Serialize, Deserialize};
use std::clone::Clone;
use std::time::Duration;

#[derive(Serialize, Deserialize)]
pub struct JsonRpc {
    jsonrpc : String,
    id : String,
    method : String,
    params : Vec<String>
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
    /// * `params` list of parameters passed to the method
    pub fn new(jsonrpc : String, id : String, method : String, params : Vec<String>) -> JsonRpc {
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
    pub fn execute(&self) -> Result<HttpResponse,&'static str> {
        if self.response_stub.is_some() {
            let r = self.response_stub.as_ref().unwrap();
            Ok(HttpResponse {
                status : r.status,
                content : r.content.clone()
            })
        }
        else {
            let client = reqwest::blocking::Client::new();
        if self.method == HttpMethod::POST {
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
            if status.is_success() {
                let content = res.text()
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
        else {
            Err("Http method currently not implemented")
        }
        }
    }

}