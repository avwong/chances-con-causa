# Chances con causa

Mi primer contrato inteligente en Stellar (Soroban, Rust).
Es una rifa con causa: se venden números y una parte de lo recaudado va para una causa.

## El problema

En Costa Rica es muy común hacer rifas para ayudar a alguien.
Por ejemplo, para un tratamiento médico, un comedor infantil o un refugio de animales.
Casi siempre se organizan por WhatsApp, con una lista en una foto o en un papel.

Eso trae problemas:

- Dos personas pueden quedar con el mismo número.
- No es claro qué números ya se pagaron.
- No hay forma de revisar que el sorteo fue justo.
- No se sabe cuánto se ha recaudado para la causa.

## Por qué lo elegí

> **Suposición:** escribí esta razón como borrador. Cámbiala por la tuya.

Elegí este problema porque las rifas con causa son parte de la vida en Costa Rica y casi todo el mundo ha comprado un número alguna vez.
Es un problema pequeño, pero real.
Un contrato inteligente sirve bien aquí: deja un registro público de los números y hace el sorteo con reglas que nadie puede cambiar a escondidas.

## Cómo funciona

Cada contrato desplegado maneja **una sola rifa**.
El dinero **no** pasa por el contrato: se paga por fuera (por ejemplo, SINPE Móvil).
El contrato solo lleva el registro y hace el sorteo.

### Flujo

1. El organizador crea la rifa: título y descripción de la causa, precio, porcentaje para la causa, fecha límite y cantidad de números.
2. Un comprador reserva un número libre. El número queda **pendiente**.
3. El comprador paga por fuera y le manda el comprobante al organizador.
4. El organizador revisa el pago:
   - Si está bien, lo **aprueba** y el número queda **pagado**.
   - Si no, lo **rechaza** y el número vuelve a estar **libre**.
5. Al pasar la fecha límite ya no se puede reservar.
6. El organizador hace el **sorteo**. Solo participan los números pagados.

### Estados de un número

| Estado | Qué significa |
| --- | --- |
| `Libre` | Nadie lo tiene. Se puede reservar. |
| `Pendiente` | Alguien lo reservó. Falta que el organizador confirme el pago. |
| `Pagado` | El organizador confirmó el pago. Participa en el sorteo. |
| `PorReembolsar` | La rifa se canceló. Hay que devolverle el dinero a esa persona. |

### Funciones

| Función | Quién firma | Qué hace |
| --- | --- | --- |
| `crear_rifa` | Organizador | Crea la rifa. Solo se puede una vez por contrato. |
| `reservar` | Comprador | Reserva un número libre. Queda pendiente. |
| `aprobar` | Organizador | Confirma el pago. El número pasa a pagado. |
| `rechazar` | Organizador | Rechaza una reserva. El número vuelve a libre. |
| `reembolsar` | Organizador | Marca que devolvió el dinero de un número pagado. El número vuelve a libre. |
| `cancelar` | Organizador | Cancela la rifa. Los números pagados quedan por reembolsar. |
| `sortear` | Organizador | Elige un ganador entre los números pagados. Solo después de la fecha límite. |
| `estado` | Nadie | Consulta el estado de un número. |
| `info` | Nadie | Muestra los datos de la rifa y el ganador, si ya hay. |
| `progreso` | Nadie | Muestra números pagados, porcentaje completado, total recaudado y cuánto va para la causa. |

Las firmas se piden con `require_auth()`.
Una persona puede reservar varios números.

### Números

Por defecto la rifa tiene 100 números, del 1 al 100.
El organizador puede escoger otra cantidad al crearla (máximo 1000).

### Cuánto va para la causa

`progreso` calcula todo con los números pagados:

- **Recaudado** = números pagados × precio.
- **Para la causa** = recaudado × porcentaje de la causa ÷ 100.
- **Porcentaje completado** = números pagados × 100 ÷ cantidad de números.

## Decisiones de seguridad y privacidad

- **No se guarda nombre ni teléfono.** Todo lo que se guarda en una blockchain es público y no se puede borrar. Si guardara esos datos, cualquiera podría verlos. El contrato solo guarda la dirección (wallet) de quien reserva. El organizador maneja los datos de contacto por fuera.
- **No se guardan comprobantes ni referencias de SINPE.** Por la misma razón. El organizador los revisa por fuera.
- **Solo el organizador puede aprobar, rechazar, reembolsar, cancelar y sortear.** El contrato lo revisa con su firma.
- **Solo el comprador puede reservar a su nombre.** Nadie puede reservar con la wallet de otra persona.

## Sobre el sorteo (versión de práctica)

El sorteo usa el generador de números aleatorios de Soroban (`env.prng()`).
Es suficiente para practicar, pero **no** es seguro para una rifa con dinero real.
Quien envía la transacción podría intentar influir en el resultado.
Una versión real necesitaría una fuente de aleatoriedad más fuerte, por ejemplo un oráculo o un esquema de compromiso y revelación.

## Suposiciones

Esta es una primera versión. Estas son las suposiciones que hice:

- El sorteo solo se puede hacer **después** de la fecha límite.
- La fecha límite es una marca de tiempo Unix (segundos), la misma que usa el ledger.
- El precio es un número entero en colones y es solo informativo, porque el pago es por fuera.
- El organizador puede aprobar o rechazar números pendientes aun después de la fecha límite, siempre que no haya hecho el sorteo.
- Los números que siguen pendientes al sortear no participan.
- Al cancelar, los números pendientes se quedan pendientes. El organizador puede rechazarlos para dejarlos libres.
- Los números por reembolsar se siguen contando en `progreso` hasta que el organizador marca el reembolso. Así se ve cuánto dinero falta devolver.
- Después del sorteo ya no se puede reembolsar ni cancelar.
- Una vez hecho el sorteo, el ganador queda guardado y no se puede volver a sortear.

## Cómo correrlo

### Requisitos

- [Rust](https://www.rust-lang.org/tools/install)
- [Stellar CLI](https://developers.stellar.org/docs/tools/cli)
- El target de WebAssembly para compilar el contrato:

```bash
rustup target add wasm32v1-none
```

### Pruebas

```bash
cargo test
```

### Compilar

```bash
stellar contract build
```

### Desplegar y usar en testnet

```bash
# Crear y fondear cuentas de prueba
stellar keys generate organizador --network testnet --fund
stellar keys generate ana --network testnet --fund

# Desplegar el contrato
stellar contract deploy \
  --wasm target/wasm32v1-none/release/chances_con_causa.wasm \
  --source organizador --network testnet --alias rifa

# Crear la rifa (100 números, 2 000 colones, 50 % para la causa)
# fecha-limite: marca de tiempo Unix, por ejemplo 1798761600 = 1 de enero de 2027 UTC
stellar contract invoke --id rifa --source organizador --network testnet -- \
  crear_rifa --organizador organizador \
  --titulo "Comedor infantil" \
  --descripcion "Almuerzos para niños de la comunidad" \
  --precio 2000 --porcentaje_causa 50 --fecha_limite 1798761600

# Ana reserva el número 7
stellar contract invoke --id rifa --source ana --network testnet -- \
  reservar --comprador ana --numero 7

# El organizador aprueba el pago
stellar contract invoke --id rifa --source organizador --network testnet -- \
  aprobar --numero 7

# Consultar
stellar contract invoke --id rifa --source ana --network testnet -- estado --numero 7
stellar contract invoke --id rifa --source ana --network testnet -- progreso

# Después de la fecha límite: sortear
stellar contract invoke --id rifa --source organizador --network testnet -- sortear
```

Para escoger otra cantidad de números, agrega `--cantidad 50` al crear la rifa.

## Pruebas

Las pruebas están en `src/test.rs`. Son 22 y revisan:

- **Crear la rifa:** 100 números por defecto, cantidad personalizada, que no se cree dos veces y que rechace datos inválidos.
- **Reservar:** queda pendiente, pide la firma del comprador, falla sin firma, no deja reservar un número ocupado ni fuera de rango, una persona puede tener varios números y no deja reservar después de la fecha límite.
- **Aprobar, rechazar y reembolsar:** aprobar marca pagado y pide la firma del organizador, no se aprueba un número libre, rechazar y reembolsar dejan el número libre otra vez y no se reembolsa un número sin pagar.
- **Consultas:** no piden firma, y `progreso` calcula bien lo recaudado y la parte para la causa.
- **Cancelar:** los pagados quedan por reembolsar y ya no se puede reservar ni sortear.
- **Sorteo:** no se hace antes de la fecha límite ni sin números pagados, solo elige números pagados y no se puede repetir.

## Estructura

```
chances-con-causa/
├── Cargo.toml
├── src/
│   ├── lib.rs    # el contrato
│   └── test.rs   # las pruebas
└── README.md
```

Basado en la estructura del ejemplo `contador` del Pura Vida Bootcamp.
