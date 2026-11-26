#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, String};

// Hora "actual" de las pruebas y fecha límite de la rifa (en segundos).
const AHORA: u64 = 1_000;
const FECHA_LIMITE: u64 = 5_000;
const PRECIO: i128 = 2_000; // 2 000 colones por número
const PORCENTAJE_CAUSA: u32 = 50;

// Crea el contrato y una rifa de prueba con 100 números.
// Devuelve el cliente y la dirección del organizador.
fn preparar(env: &Env) -> (ChancesConCausaClient<'_>, Address) {
    env.mock_all_auths();
    env.ledger().set_timestamp(AHORA);

    let id = env.register(ChancesConCausa, ());
    let cliente = ChancesConCausaClient::new(env, &id);
    let organizador = Address::generate(env);

    cliente.crear_rifa(
        &organizador,
        &String::from_str(env, "Comedor infantil"),
        &String::from_str(env, "Almuerzos para niños de la comunidad"),
        &PRECIO,
        &PORCENTAJE_CAUSA,
        &FECHA_LIMITE,
        &None,
    );
    (cliente, organizador)
}

// Reserva y aprueba un número de una vez.
fn comprar(cliente: &ChancesConCausaClient, comprador: &Address, numero: u32) {
    cliente.reservar(comprador, &numero);
    cliente.aprobar(&numero);
}

// ----- Crear la rifa -----

#[test]
fn crea_rifa_con_100_numeros_por_defecto() {
    let env = Env::default();
    let (cliente, organizador) = preparar(&env);

    let rifa = cliente.info();
    assert_eq!(rifa.organizador, organizador);
    assert_eq!(rifa.titulo, String::from_str(&env, "Comedor infantil"));
    assert_eq!(rifa.cantidad, 100);
    assert_eq!(rifa.estado, EstadoRifa::Activa);
    assert_eq!(rifa.ganador, None);
}

#[test]
fn el_organizador_puede_escoger_la_cantidad() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(ChancesConCausa, ());
    let cliente = ChancesConCausaClient::new(&env, &id);
    let organizador = Address::generate(&env);
    let texto = String::from_str(&env, "Causa");

    cliente.crear_rifa(&organizador, &texto, &texto, &PRECIO, &10, &FECHA_LIMITE, &Some(50));

    assert_eq!(cliente.info().cantidad, 50);
    assert_eq!(cliente.estado(&50), EstadoNumero::Libre);
    assert_eq!(cliente.try_estado(&51), Err(Ok(Error::NumeroInvalido)));
}

#[test]
fn no_se_puede_crear_dos_veces() {
    let env = Env::default();
    let (cliente, organizador) = preparar(&env);
    let texto = String::from_str(&env, "Otra");

    let resultado =
        cliente.try_crear_rifa(&organizador, &texto, &texto, &PRECIO, &10, &FECHA_LIMITE, &None);
    assert_eq!(resultado, Err(Ok(Error::YaExisteRifa)));
}

#[test]
fn rechaza_datos_invalidos() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(AHORA);
    let id = env.register(ChancesConCausa, ());
    let cliente = ChancesConCausaClient::new(&env, &id);
    let organizador = Address::generate(&env);
    let texto = String::from_str(&env, "Causa");

    // Porcentaje mayor a 100
    let r = cliente.try_crear_rifa(&organizador, &texto, &texto, &PRECIO, &101, &FECHA_LIMITE, &None);
    assert_eq!(r, Err(Ok(Error::DatosInvalidos)));
    // Precio en cero
    let r = cliente.try_crear_rifa(&organizador, &texto, &texto, &0, &10, &FECHA_LIMITE, &None);
    assert_eq!(r, Err(Ok(Error::DatosInvalidos)));
    // Fecha límite en el pasado
    let r = cliente.try_crear_rifa(&organizador, &texto, &texto, &PRECIO, &10, &(AHORA - 1), &None);
    assert_eq!(r, Err(Ok(Error::DatosInvalidos)));
}

// ----- Reservar -----

#[test]
fn reservar_deja_el_numero_pendiente() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    cliente.reservar(&ana, &7);
    assert_eq!(cliente.estado(&7), EstadoNumero::Pendiente(ana));
}

#[test]
fn reservar_pide_la_firma_del_comprador() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    cliente.reservar(&ana, &7);
    assert_eq!(env.auths()[0].0, ana);
}

#[test]
#[should_panic]
fn reservar_sin_firma_falla() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    env.set_auths(&[]); // quitamos las firmas simuladas
    cliente.reservar(&ana, &7);
}

#[test]
fn no_se_reserva_un_numero_ocupado() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);
    let beto = Address::generate(&env);

    cliente.reservar(&ana, &7);
    assert_eq!(cliente.try_reservar(&beto, &7), Err(Ok(Error::NumeroNoDisponible)));
}

#[test]
fn no_se_reserva_un_numero_fuera_de_rango() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    assert_eq!(cliente.try_reservar(&ana, &0), Err(Ok(Error::NumeroInvalido)));
    assert_eq!(cliente.try_reservar(&ana, &101), Err(Ok(Error::NumeroInvalido)));
}

#[test]
fn una_persona_puede_tener_varios_numeros() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    cliente.reservar(&ana, &1);
    cliente.reservar(&ana, &2);
    assert_eq!(cliente.estado(&1), EstadoNumero::Pendiente(ana.clone()));
    assert_eq!(cliente.estado(&2), EstadoNumero::Pendiente(ana));
}

#[test]
fn no_se_reserva_despues_de_la_fecha_limite() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    env.ledger().set_timestamp(FECHA_LIMITE);
    assert_eq!(cliente.try_reservar(&ana, &7), Err(Ok(Error::FechaVencida)));
}

// ----- Aprobar, rechazar y reembolsar -----

#[test]
fn aprobar_marca_el_numero_como_pagado() {
    let env = Env::default();
    let (cliente, organizador) = preparar(&env);
    let ana = Address::generate(&env);

    cliente.reservar(&ana, &7);
    cliente.aprobar(&7);

    assert_eq!(env.auths()[0].0, organizador); // firmó el organizador
    assert_eq!(cliente.estado(&7), EstadoNumero::Pagado(ana));
}

#[test]
fn no_se_aprueba_un_numero_libre() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);

    assert_eq!(cliente.try_aprobar(&7), Err(Ok(Error::NumeroNoPendiente)));
}

#[test]
fn rechazar_deja_el_numero_libre_otra_vez() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);
    let beto = Address::generate(&env);

    cliente.reservar(&ana, &7);
    cliente.rechazar(&7);
    assert_eq!(cliente.estado(&7), EstadoNumero::Libre);

    // Otra persona ya lo puede reservar
    cliente.reservar(&beto, &7);
    assert_eq!(cliente.estado(&7), EstadoNumero::Pendiente(beto));
}

#[test]
fn reembolsar_deja_el_numero_libre_otra_vez() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    comprar(&cliente, &ana, 7);
    cliente.reembolsar(&7);

    assert_eq!(cliente.estado(&7), EstadoNumero::Libre);
    assert_eq!(cliente.progreso().pagados, 0);
}

#[test]
fn no_se_reembolsa_un_numero_sin_pagar() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    cliente.reservar(&ana, &7);
    assert_eq!(cliente.try_reembolsar(&7), Err(Ok(Error::NumeroNoPagado)));
}

// ----- Consultas -----

#[test]
fn consultar_no_pide_firma() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);

    env.set_auths(&[]);
    assert_eq!(cliente.estado(&7), EstadoNumero::Libre);
    assert_eq!(cliente.info().cantidad, 100);
    assert_eq!(cliente.progreso().pagados, 0);
}

#[test]
fn progreso_calcula_lo_recaudado_y_la_causa() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    comprar(&cliente, &ana, 1);
    comprar(&cliente, &ana, 2);
    comprar(&cliente, &ana, 3);
    cliente.reservar(&ana, &4); // pendiente: no cuenta

    let progreso = cliente.progreso();
    assert_eq!(progreso.pagados, 3);
    assert_eq!(progreso.porcentaje_completado, 3); // 3 de 100
    assert_eq!(progreso.recaudado, 6_000); // 3 * 2 000
    assert_eq!(progreso.para_la_causa, 3_000); // 50 %
}

// ----- Cancelar -----

#[test]
fn cancelar_marca_los_pagados_por_reembolsar() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    comprar(&cliente, &ana, 7);
    cliente.cancelar();

    assert_eq!(cliente.info().estado, EstadoRifa::Cancelada);
    assert_eq!(cliente.estado(&7), EstadoNumero::PorReembolsar(ana.clone()));

    // Ya no se puede reservar ni sortear
    assert_eq!(cliente.try_reservar(&ana, &8), Err(Ok(Error::RifaNoActiva)));
    env.ledger().set_timestamp(FECHA_LIMITE);
    assert_eq!(cliente.try_sortear(), Err(Ok(Error::RifaNoActiva)));

    // Cuando el organizador devuelve el dinero, el número queda libre
    cliente.reembolsar(&7);
    assert_eq!(cliente.estado(&7), EstadoNumero::Libre);
}

// ----- Sorteo -----

#[test]
fn no_se_sortea_antes_de_la_fecha_limite() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    comprar(&cliente, &ana, 7);
    assert_eq!(cliente.try_sortear(), Err(Ok(Error::SorteoAntesDeTiempo)));
}

#[test]
fn no_se_sortea_sin_numeros_pagados() {
    let env = Env::default();
    let (cliente, _) = preparar(&env);
    let ana = Address::generate(&env);

    cliente.reservar(&ana, &7); // solo pendiente
    env.ledger().set_timestamp(FECHA_LIMITE);
    assert_eq!(cliente.try_sortear(), Err(Ok(Error::SinNumerosPagados)));
}

#[test]
fn el_sorteo_solo_elige_numeros_pagados() {
    let env = Env::default();
    let (cliente, organizador) = preparar(&env);
    let ana = Address::generate(&env);
    let beto = Address::generate(&env);

    comprar(&cliente, &ana, 3);
    comprar(&cliente, &beto, 9);
    cliente.reservar(&beto, &5); // pendiente: no participa

    env.ledger().set_timestamp(FECHA_LIMITE);
    let ganador = cliente.sortear();

    assert_eq!(env.auths()[0].0, organizador);
    assert!(ganador == 3 || ganador == 9);

    let rifa = cliente.info();
    assert_eq!(rifa.estado, EstadoRifa::Sorteada);
    assert_eq!(rifa.ganador, Some(ganador));

    // No se puede sortear dos veces
    assert_eq!(cliente.try_sortear(), Err(Ok(Error::RifaNoActiva)));
}
