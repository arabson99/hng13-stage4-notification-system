use actix_web::{
  dev::{Service, ServiceRequest, ServiceResponse, Transform},
  Error,
};
use actix_web::http::header::{HeaderName, HeaderValue};
use futures_util::future::{ready, Ready, LocalBoxFuture};
use std::task::{Context, Poll};

pub struct CorrelationId;

impl<S, B> Transform<S, ServiceRequest> for CorrelationId
where
  S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
  B: 'static,
{
  type Response = ServiceResponse<B>;
  type Error = Error;
  type InitError = ();
  type Transform = CorrelationIdMiddleware<S>;
  type Future = Ready<Result<Self::Transform, Self::InitError>>;

  fn new_transform(&self, service: S) -> Self::Future {
    ready(Ok(CorrelationIdMiddleware { service }))
  }
}

pub struct CorrelationIdMiddleware<S> { service: S }

impl<S, B> Service<ServiceRequest> for CorrelationIdMiddleware<S>
where
  S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
  B: 'static,
{
  type Response = ServiceResponse<B>;
  type Error = Error;
  type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

  fn poll_ready(&self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&self, mut req: ServiceRequest) -> Self::Future {
    if req.headers().get("x-correlation-id").is_none() {
      let name = HeaderName::from_static("x-correlation-id");
      let value = HeaderValue::from_str(&uuid::Uuid::new_v4().to_string()).unwrap();
      req.headers_mut().insert(name, value);
    }
    let fut = self.service.call(req);
    Box::pin(async move { let res = fut.await?; Ok(res) })
  }
}
