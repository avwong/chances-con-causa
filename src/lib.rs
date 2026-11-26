#![no_std]

// Contrato "Chances con causa": una rifa con causa en Stellar (Soroban).
//
// Cada contrato desplegado maneja UNA sola rifa.
// El dinero se paga por fuera (por ejemplo, SINPE Móvil). El contrato solo
// lleva el registro de los números y hace el sorteo.
// No se guardan nombres ni teléfonos: todo lo que está en la blockchain es público.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec,
};

// Si el organizador no indica cuántos números quiere, la rifa tiene 100 (del 1 al 100).
const CANTIDAD_PREDETERMINADA: u32 = 100;
// Límite para que la rifa no crezca demasiado.
const CANTIDAD_MAXIMA: u32 = 1000;

// Tiempo de vida de los datos (en ledgers), igual que en el ejemplo contador.
const TTL_MINIMO: u32 = 17_280; // aprox. 1 día
const TTL_EXTENDIDO: u32 = 518_400; // aprox. 30 días

// Estado general de la rifa.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EstadoRifa {
    Activa,
    Cancelada,
    Sorteada,
}

// Estado de cada número. Guarda la dirección (wallet) de quien lo reservó.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EstadoNumero {
    Libre,
    Pendiente(Address),     // reservado, esperando que el organizador confirme el pago
    Pagado(Address),        // el organizador confirmó el pago
    PorReembolsar(Address), // la rifa se canceló y hay que devolverle el dinero
}

// Datos de la rifa.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rifa {
    pub organizador: Address,
    pub titulo: String,        // título de la causa
    pub descripcion: String,   // descripción corta de la causa
    pub precio: i128,          // precio de cada número (en colones, solo informativo)
    pub porcentaje_causa: u32, // qué porcentaje de lo recaudado va para la causa
    pub fecha_limite: u64,     // marca de tiempo (segundos) en que se cierran las reservas
    pub cantidad: u32,         // cuántos números tiene la rifa (del 1 a cantidad)
    pub estado: EstadoRifa,
    pub ganador: Option<u32>, // número ganador, cuando ya se sorteó
}

// Resumen de cómo va la rifa.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Progreso {
    pub pagados: u32,               // números con pago confirmado
    pub porcentaje_completado: u32, // pagados / cantidad, de 0 a 100
    pub recaudado: i128,            // pagados * precio
    pub para_la_causa: i128,        // la parte de lo recaudado que va para la causa
}

// Claves para guardar datos en el storage.
#[contracttype]
pub enum Clave {
    Rifa,
    Pagados,     // lista de números pagados (para el sorteo)
    Numero(u32), // estado de un número
}

// Errores que puede devolver el contrato.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    YaExisteRifa = 1,
    NoExisteRifa = 2,
    DatosInvalidos = 3,
    NumeroInvalido = 4,
    NumeroNoDisponible = 5,
    FechaVencida = 6,
    RifaNoActiva = 7,
    NumeroNoPendiente = 8,
    NumeroNoPagado = 9,
    SorteoAntesDeTiempo = 10,
    SinNumerosPagados = 11,
}

#[contract]
pub struct ChancesConCausa;

#[contractimpl]
impl ChancesConCausa {
    // El organizador crea la rifa. Solo se puede hacer una vez por contrato.
    // Si `cantidad` es None, la rifa tiene 100 números.
    pub fn crear_rifa(
        env: Env,
        organizador: Address,
        titulo: String,
        descripcion: String,
        precio: i128,
        porcentaje_causa: u32,
        fecha_limite: u64,
        cantidad: Option<u32>,
    ) -> Result<(), Error> {
        organizador.require_auth();

        if env.storage().instance().has(&Clave::Rifa) {
            return Err(Error::YaExisteRifa);
        }

        let cantidad = cantidad.unwrap_or(CANTIDAD_PREDETERMINADA);
        let datos_validos = titulo.len() > 0
            && precio > 0
            && porcentaje_causa <= 100
            && fecha_limite > env.ledger().timestamp()
            && cantidad >= 1
            && cantidad <= CANTIDAD_MAXIMA;
        if !datos_validos {
            return Err(Error::DatosInvalidos);
        }

        let rifa = Rifa {
            organizador,
            titulo,
            descripcion,
            precio,
            porcentaje_causa,
            fecha_limite,
            cantidad,
            estado: EstadoRifa::Activa,
            ganador: None,
        };
        guardar_rifa(&env, &rifa);
        guardar_pagados(&env, &Vec::new(&env));
        Ok(())
    }

    // Cualquiera puede ver los datos de la rifa (no pide firma).
    pub fn info(env: Env) -> Result<Rifa, Error> {
        leer_rifa(&env)
    }

    // Un comprador reserva un número libre. Queda pendiente hasta que el
    // organizador confirme el pago. Una persona puede reservar varios números.
    pub fn reservar(env: Env, comprador: Address, numero: u32) -> Result<(), Error> {
        comprador.require_auth();

        let rifa = leer_rifa(&env)?;
        if rifa.estado != EstadoRifa::Activa {
            return Err(Error::RifaNoActiva);
        }
        if env.ledger().timestamp() >= rifa.fecha_limite {
            return Err(Error::FechaVencida);
        }
        validar_numero(&rifa, numero)?;
        if leer_numero(&env, numero) != EstadoNumero::Libre {
            return Err(Error::NumeroNoDisponible);
        }

        guardar_numero(&env, numero, &EstadoNumero::Pendiente(comprador));
        Ok(())
    }

    // El organizador confirma que recibió el pago: el número pasa a pagado.
    pub fn aprobar(env: Env, numero: u32) -> Result<(), Error> {
        let rifa = leer_rifa(&env)?;
        rifa.organizador.require_auth();

        if rifa.estado != EstadoRifa::Activa {
            return Err(Error::RifaNoActiva);
        }
        validar_numero(&rifa, numero)?;

        match leer_numero(&env, numero) {
            EstadoNumero::Pendiente(comprador) => {
                guardar_numero(&env, numero, &EstadoNumero::Pagado(comprador));
                let mut pagados = leer_pagados(&env);
                pagados.push_back(numero);
                guardar_pagados(&env, &pagados);
                Ok(())
            }
            _ => Err(Error::NumeroNoPendiente),
        }
    }

    // El organizador rechaza una reserva (por ejemplo, nunca llegó el pago).
    // El número vuelve a quedar libre.
    pub fn rechazar(env: Env, numero: u32) -> Result<(), Error> {
        let rifa = leer_rifa(&env)?;
        rifa.organizador.require_auth();
        validar_numero(&rifa, numero)?;

        match leer_numero(&env, numero) {
            EstadoNumero::Pendiente(_) => {
                borrar_numero(&env, numero);
                Ok(())
            }
            _ => Err(Error::NumeroNoPendiente),
        }
    }

    // El organizador marca que ya devolvió el dinero de un número pagado
    // (la devolución se hace por fuera). El número vuelve a quedar libre.
    pub fn reembolsar(env: Env, numero: u32) -> Result<(), Error> {
        let rifa = leer_rifa(&env)?;
        rifa.organizador.require_auth();

        if rifa.estado == EstadoRifa::Sorteada {
            return Err(Error::RifaNoActiva);
        }
        validar_numero(&rifa, numero)?;

        match leer_numero(&env, numero) {
            EstadoNumero::Pagado(_) | EstadoNumero::PorReembolsar(_) => {
                borrar_numero(&env, numero);
                let mut pagados = leer_pagados(&env);
                if let Some(i) = pagados.first_index_of(numero) {
                    pagados.remove(i);
                }
                guardar_pagados(&env, &pagados);
                Ok(())
            }
            _ => Err(Error::NumeroNoPagado),
        }
    }

    // El organizador cancela la rifa. Los números pagados quedan
    // marcados "por reembolsar" y ya no se puede reservar ni sortear.
    pub fn cancelar(env: Env) -> Result<(), Error> {
        let mut rifa = leer_rifa(&env)?;
        rifa.organizador.require_auth();

        if rifa.estado != EstadoRifa::Activa {
            return Err(Error::RifaNoActiva);
        }

        for numero in leer_pagados(&env).iter() {
            if let EstadoNumero::Pagado(comprador) = leer_numero(&env, numero) {
                guardar_numero(&env, numero, &EstadoNumero::PorReembolsar(comprador));
            }
        }

        rifa.estado = EstadoRifa::Cancelada;
        guardar_rifa(&env, &rifa);
        Ok(())
    }

    // Cualquiera puede consultar el estado de un número (no pide firma).
    pub fn estado(env: Env, numero: u32) -> Result<EstadoNumero, Error> {
        let rifa = leer_rifa(&env)?;
        validar_numero(&rifa, numero)?;
        Ok(leer_numero(&env, numero))
    }

    // Cuánto se ha vendido y cuánto va para la causa (no pide firma).
    pub fn progreso(env: Env) -> Result<Progreso, Error> {
        let rifa = leer_rifa(&env)?;
        let pagados = leer_pagados(&env).len();
        let recaudado = rifa.precio * pagados as i128;

        Ok(Progreso {
            pagados,
            porcentaje_completado: pagados * 100 / rifa.cantidad,
            recaudado,
            para_la_causa: recaudado * rifa.porcentaje_causa as i128 / 100,
        })
    }

    // El organizador hace el sorteo cuando ya pasó la fecha límite.
    // Solo participan los números pagados. Devuelve el número ganador.
    pub fn sortear(env: Env) -> Result<u32, Error> {
        let mut rifa = leer_rifa(&env)?;
        rifa.organizador.require_auth();

        if rifa.estado != EstadoRifa::Activa {
            return Err(Error::RifaNoActiva);
        }
        if env.ledger().timestamp() < rifa.fecha_limite {
            return Err(Error::SorteoAntesDeTiempo);
        }

        let pagados = leer_pagados(&env);
        if pagados.is_empty() {
            return Err(Error::SinNumerosPagados);
        }

        // Número aleatorio de Soroban (versión de práctica, ver README).
        let posicion = env.prng().gen_range::<u64>(0..=(pagados.len() as u64 - 1));
        let ganador = pagados.get(posicion as u32).unwrap();

        rifa.estado = EstadoRifa::Sorteada;
        rifa.ganador = Some(ganador);
        guardar_rifa(&env, &rifa);
        Ok(ganador)
    }
}

// ----- Funciones de ayuda (no se pueden llamar desde fuera del contrato) -----

fn leer_rifa(env: &Env) -> Result<Rifa, Error> {
    env.storage()
        .instance()
        .get(&Clave::Rifa)
        .ok_or(Error::NoExisteRifa)
}

fn guardar_rifa(env: &Env, rifa: &Rifa) {
    env.storage().instance().set(&Clave::Rifa, rifa);
    env.storage().instance().extend_ttl(TTL_MINIMO, TTL_EXTENDIDO);
}

fn leer_pagados(env: &Env) -> Vec<u32> {
    env.storage()
        .instance()
        .get(&Clave::Pagados)
        .unwrap_or(Vec::new(env))
}

fn guardar_pagados(env: &Env, pagados: &Vec<u32>) {
    env.storage().instance().set(&Clave::Pagados, pagados);
    env.storage().instance().extend_ttl(TTL_MINIMO, TTL_EXTENDIDO);
}

// Si un número no está guardado, está libre.
fn leer_numero(env: &Env, numero: u32) -> EstadoNumero {
    env.storage()
        .persistent()
        .get(&Clave::Numero(numero))
        .unwrap_or(EstadoNumero::Libre)
}

fn guardar_numero(env: &Env, numero: u32, estado: &EstadoNumero) {
    let clave = Clave::Numero(numero);
    env.storage().persistent().set(&clave, estado);
    env.storage()
        .persistent()
        .extend_ttl(&clave, TTL_MINIMO, TTL_EXTENDIDO);
}

// Borrar el número lo deja libre otra vez.
fn borrar_numero(env: &Env, numero: u32) {
    env.storage().persistent().remove(&Clave::Numero(numero));
}

// Los números válidos van del 1 hasta la cantidad de la rifa.
fn validar_numero(rifa: &Rifa, numero: u32) -> Result<(), Error> {
    if numero < 1 || numero > rifa.cantidad {
        return Err(Error::NumeroInvalido);
    }
    Ok(())
}
