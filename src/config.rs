use std::env;

pub struct Config {
    pub agus_dev_sso_host: String,
    pub port: u16,
    pub postgres_url: String,
    pub kvs_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl Config {
    pub fn read_env() -> Self {
        Config {
            agus_dev_sso_host: env::var("AGUS_DEV_SSO_HOST")
                .unwrap_or(String::from("https://sso.v2.agus.dev")),
            port: env::var("SERVER_PORT")
                .expect("SERVER_PORT must be set")
                .parse()
                .expect("SERVER_PORT must be a number"),
            postgres_url: env::var("POSTGRES_URL").expect("POSTGRES_URL must be set"),
            kvs_url: env::var("KVS_URL").expect("KVS_URL must be set"),
            client_id: env::var("CLIENT_ID").expect("CLIENT_ID must be set"),
            client_secret: env::var("CLIENT_SECRET").expect("CLIENT_SECRET must be set"),
            redirect_uri: env::var("REDIRECT_URI").expect("REDIRECT_URI must be set"),
        }
    }
}
