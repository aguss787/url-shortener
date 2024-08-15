use std::error::Error;

pub type KvsPool = deadpool_redis::Pool;
pub type KvsPoolError = deadpool_redis::PoolError;
pub type KvsError = redis::RedisError;

pub fn kvs_pool(host: &str) -> Result<KvsPool, Box<dyn Error>> {
    let cfg = deadpool_redis::Config::from_url(host);
    cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .map_err(Into::into)
}
