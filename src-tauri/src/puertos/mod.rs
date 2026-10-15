// Puertos: contratos que el dominio define y la infraestructura implementa.

pub mod reloj;
pub mod repositorios;

#[cfg(test)]
pub mod contrato;
#[cfg(test)]
pub mod dobles;
