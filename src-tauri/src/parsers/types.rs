use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Log<M: Serialize, C: Serialize, D: Serialize> {
  pub meta: M,
  pub channels: Vec<C>,
  pub data: Vec<D>,
}

pub trait Parser<M: Serialize, C: Serialize, D: Serialize> {
  fn parse(&self, data: &str) -> Result<Log<M, C, D>, Box<dyn std::error::Error>>;
}