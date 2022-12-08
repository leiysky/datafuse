use poem::{
    async_trait, get, handler, listener::TcpListener, Middleware, Request, Response, Result, Endpoint,IntoResponse,
    Server,
};
use crate::models::Credentials;

pub struct SharingAuth;

impl<E: Endpoint> Middleware<E> for SharingAuth {
    type Output = SharingAuthImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        SharingAuthImpl(ep)
    }
}

pub struct SharingAuthImpl<E>(E);



#[async_trait]
impl<E: Endpoint> Endpoint for SharingAuthImpl<E> {
    type Output = Response;

    // TODO(zhihanz) current implementation only used for stateless test
    // for production usage, we need to implement a middleware with JWT authentication
    async fn call(&self, mut req: Request) -> Result<Self::Output> {
        println!("req: {:?}", req);

        // decode auth header from bearer base64
        let auth_header = req.headers().get("Authorization").unwrap().to_str().unwrap();
        let auth_header = auth_header.split(" ").collect::<Vec<&str>>();
        let auth_header = auth_header[1];
        let auth_header = base64::decode(auth_header).unwrap();
        let auth_header = String::from_utf8(auth_header).unwrap();
        println!("auth_header: {:?}", auth_header);
        req.extensions_mut().insert(Credentials { token: auth_header });
        // add json content type if not provided
        if req.headers().get("Content-Type").is_none() {
            req.headers_mut().insert("Content-Type", "application/json".parse().unwrap());
        }

        let res = self.0.call(req).await;
        match res {
            Ok(resp) => {
                let resp = resp.into_response();
                Ok(resp)
            }
            Err(err) => {
                println!("err: {:?}", err);
                Err(err)
            },
        }
    }
}