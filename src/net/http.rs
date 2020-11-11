pub struct HttpRequest {
    url : String,
    method : String,
    headers : Vec<String>,
    body : String
}

impl HttpRequest {
    pub fn new(url : String, method : String, headers : Vec<String>, body : String) -> HttpRequest {
        HttpRequest {
            url : url,
            method : method,
            headers : headers,
            body : body
        }
    }

    pub fn execute(&self) {

    }
}