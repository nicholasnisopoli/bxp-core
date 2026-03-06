// src/router.rs
use crate::protocol::{BxpAction, BxpRequest, BxpStatus};
use crate::server::BxpServerConnection;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

// 1. The Object-Safe Trait (Used to store handlers in the HashMap)
pub trait BxpHandler: Send + Sync {
    fn call<'a>(&'a self, req: BxpRequest, conn: &'a mut BxpServerConnection) -> BoxFuture<'a>;
}

// 2. The Helper Trait (Extracts lifetimes automatically from async functions)
pub trait AsyncFnHandler<'a>: Send + Sync {
    type Fut: Future<Output = anyhow::Result<()>> + Send + 'a;
    fn call(&self, req: BxpRequest, conn: &'a mut BxpServerConnection) -> Self::Fut;
}

// 3. Implement the Helper Trait for all compatible Rust functions
impl<'a, F, Fut> AsyncFnHandler<'a> for F
where
    F: Fn(BxpRequest, &'a mut BxpServerConnection) -> Fut + Send + Sync,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'a,
{
    type Fut = Fut;
    fn call(&self, req: BxpRequest, conn: &'a mut BxpServerConnection) -> Self::Fut {
        (self)(req, conn)
    }
}

// 4. The Bridge! If a function satisfies the Helper Trait, it becomes a BxpHandler!
impl<F> BxpHandler for F
where
    F: for<'a> AsyncFnHandler<'a> + 'static,
{
    fn call<'a>(&'a self, req: BxpRequest, conn: &'a mut BxpServerConnection) -> BoxFuture<'a> {
        // Automatically box the future here, keeping it hidden from the user!
        Box::pin(AsyncFnHandler::call(self, req, conn))
    }
}

#[derive(Default)]
pub struct BxpRouter {
    routes: HashMap<BxpAction, HashMap<String, Box<dyn BxpHandler>>>,
}
/// The BxpRouter struct and its methods for registering routes and handling requests.
impl BxpRouter {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }
    /// Registers a handler function for a specific action and URI pattern. The handler can be any async function that takes a BxpRequest and a mutable reference to BxpServerConnection, and returns a Result. The `route` method is designed to be ergonomic, allowing users to easily register handlers without worrying about the underlying trait object mechanics.
    pub fn route<H>(mut self, action: BxpAction, uri: &str, handler: H) -> Self
    where
        H: BxpHandler + 'static,
    {
        self.routes
            .entry(action)
            .or_insert_with(HashMap::new)
            .insert(uri.to_string(), Box::new(handler));
        self
    }
    /// Handles an incoming request by looking up the appropriate handler based on the action and URI, and then calling it. If no handler is found, it sends a 404 Not Found response.
    pub async fn handle_request(
        &self,
        req: BxpRequest,
        conn: &mut BxpServerConnection,
    ) -> anyhow::Result<()> {
        if let Some(handler) = self.routes.get(&req.action).and_then(|m| m.get(req.uri.as_str())) {
            handler.call(req, conn).await?;
        } else {
            conn.send_response(req.req_id, BxpStatus::NotFound).await?;
        }
        Ok(())
    }
}