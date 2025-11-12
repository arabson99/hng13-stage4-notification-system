use actix_web::{dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, Error, http::header};
use futures_util::future::{ok, Ready, LocalBoxFuture};
use uuid::Uuid;

pub struct CorrelationId;

impl<S, B> Transform<S, ServiceRequest> for CorrelationId
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = CorrelationIdMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CorrelationIdMiddleware { service })
    }
}

pub struct CorrelationIdMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for CorrelationIdMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let header_name = header::HeaderName::from_static("x-correlation-id");
        let cid = req.headers().get(&header_name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        req.headers_mut().insert(
            header_name.clone(),
            header::HeaderValue::from_str(&cid).unwrap_or_else(|_| header::HeaderValue::from_static("invalid")),
        );

        let fut = self.service.call(req);
        Box::pin(async move {
            let mut res = fut.await?;
            res.headers_mut().insert(
                header_name,
                header::HeaderValue::from_str(&cid).unwrap_or_else(|_| header::HeaderValue::from_static("invalid")),
            );
            Ok(res)
        })
    }
}
