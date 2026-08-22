# Roadmap

Plan de trabajo de ~22 semanas. Este archivo es la memoria del proyecto: si algo
no está aquí, se va a olvidar.

---

## 1. Objetivo

Un path tracer distribuido que sirva como portafolio para maestrías en HPC.
El proyecto FPGA/HLS/SoC ya cubre la casilla de "bajo nivel y hardware", así que
lo que este tiene que demostrar es **sistemas más grandes que un chip**: construir
algo sustancial y complejo, y después escalarlo con datos.

La narrativa que hay que poder contar al final:

> Optimizo cómputo desde la puerta lógica hasta el cluster: diseñé aceleradores en
> HLS sobre SoC, llevé un path tracer de X a Y trabajando la jerarquía de memoria
> y SIMD, y luego lo escalé a N nodos analizando desbalanceo y tolerancia a fallos.

**Hito visual intermedio:** una mesa de resina epoxi + madera (el efecto "río de
lava"). Requiere ruido procedural con domain warping, absorción Beer-Lambert,
clearcoat, HDRI y cáusticas. Cruza las fases 2 y 3.

**El riesgo real no es que el proyecto no alcance: es no terminarlo.** Proteger la
Fase 6 y el reporte por encima de todo. Un renderer al 80% con paper vale más que
uno al 100% sin él.

---

## 2. Estado actual

| | Actividad | Estado |
|---|---|---|
| F0.1 | Perfil de release + `target-cpu` | ✅ |
| F0.2 | Framebuffer HDR + transform de display separada + export EXR | ✅ |
| F0.3a | `rt-bench`: crate, manifiesto, wall time, metadata, JSONL | ✅ |
| F0.3b | `RayStats`, rayos/s, samples/s, histograma por tile | ✅ |
| F0.3c | `rt-bench reference` (EXR a 25k spp / `d100`), MSE y eficiencia | ✅ |
| F0.4 | Determinismo por (píxel, sample) + test de regresión | ✅ |
| F0.5 | Manejo de errores, `parking_lot` | ✅ |
| F0.6 | `Material` y primitivas a enums, material por índice | ✅ |
| F0.7 | BVH: eje de mayor extensión, sin duplicados, front-to-back | ✅ |
| F0.8 | BVH plano + `Aabb{min,max}` + slab SIMD + `node_visits`/`prim_tests` | ✅ |
| F0.9 | Ruleta rusa, `tile_size` 32, `max_depth` 64, `random_unit_vector` analítico | ✅ |
| F0.10 | `rt-bench preview` + `--tracer normal` como benchmark de recorrido | ✅ |
| F0.11 | White furnace test automatizado (3 casos) | ✅ |
| F0.12 | `rt-bench ceilings` + roofline con modelo auditable | ✅ |
| — | Barrida histórica: 19 commits × 2 escenas × 2 configs | ✅ |
| — | Generaciones de hardware (`bench/hardware.toml`) + guarda de binario obsoleto | ✅ |
| — | Suite B1 + B2 congelada, baselines en `bench/history.jsonl` | ✅ |
| — | Tests estables (62, 1 ignorado) | ✅ |
| F1.0 | `rt-bench converge` + figura de convergencia MSE vs tiempo | ✅ |

**La Fase 0 está cerrada.** F0.13 se movió a F2.5b y F0.14 a F4.7b: ninguna
bloqueaba la Fase 1, y tenerlas acá solo retrasaba el salto de calidad.

Rendimiento acumulado sobre los 19 commits: **×2.94 en B1 y ×10.84 en B2**, con
la contribución de cada cambio atribuida y su dispersión.

F1 ya tiene su instrumento (F1.0) y arranca en F1.1. Desde acá la métrica deja de
ser el reloj — ver §3.6 y §10.

---

## 3. Hallazgos que dieron forma al plan

Estos son datos medidos, no suposiciones. Si el plan parece raro en algún punto,
la razón está acá.

### 3.1 Contribución de cada cambio (B2, 1920×250, mediana de 3 reps)

| Commit | Tiempo | vs anterior |
|---|---|---|
| pre-bvh | 170.7 s | — |
| bvh-enabled | 56.5 s | **×3.02** |
| release-profile | 59.0 s | ×0.96 |
| bvh-axis-fix | 37.0 s | **×1.59** |
| framebuffer-f32 | 38.2 s | ×0.97 |
| head | 37.7 s | ×1.01 |
| | | **total ×4.53** |

### 3.2 El perfil de compilación no hizo nada

`lto=fat` + `codegen-units=1` + `target-cpu=native` dieron **×1.00**, no el
1.2–1.5× que se estimó. Medido con perfil por defecto vs optimizado en los 6
commits, incluso en las filas de baja varianza.

Hipótesis: **el recorrido está limitado por latencia de memoria**, no por cómputo
— punteros `Arc<dyn Hittable>` dispersos en el heap con una llamada virtual por
nodo. Mejor codegen no ayuda a un bucle que espera la caché.

**Consecuencia:** el BVH plano con primitivas contiguas (originalmente F2.3) sube
de prioridad por encima del slab test SIMD, que podría decepcionar por la misma
razón.

**Confirmado en parte.** El BVH plano dio ×1.51 en B1 y ×1.57 en B2, lo previsto.
Pero el slab SIMD **no decepcionó**: ×1.07 y ×1.11, cuando la estimación era
0–7% con la evidencia apuntando al piso. El desensamblado explica por qué la
estimación quedó corta: LLVM ya había vuelto branchless el swap de signo con
`cmpltss`+`blendvps`, así que no había ramas que ganar — pero la aritmética
seguía siendo escalar (`vsubss`/`vmulss`), y vectorizarla rindió más de lo
proyectado.

Evidencia de apoyo: en B1 (19 objetos) el BVH **cuesta ~7%** contra un barrido
lineal — `pre-bvh [21.8, 22.8, 22.5]` vs `bvh [23.7, 24.2, 25.2]`, sin solaparse.
Ese ×0.93 es literalmente el costo de la indirección.

### 3.3 El BVH es no determinista y eso bloquea la medición

`BvhNode::new` elige el eje de corte con `fastrand`, que se siembra por proceso.
**Cada corrida construye un árbol estructuralmente distinto**, con distinto costo
de recorrido.

Prueba: en B1, `pre-bvh` (barrido lineal, determinista) tiene ±2% de dispersión;
**todos** los commits con BVH tienen 10–25%. Con 19 objetos y ~1 s de trabajo, los
térmicos afectarían a todas las filas por igual. No lo hacen.

También apareció como **tests intermitentes**: `test_bvh_traversal_hit_and_short_circuit`
fallaba ~40% de las corridas.

**Consecuencia:** F0.7 (split por eje más largo) sube al primer lugar. Hoy no se
puede atribuir nada por debajo de ~30% en escenas con BVH, así que F0.6, F0.8 y
F0.9 quedarían enterradas en ruido. También impidió medir el overhead de la
instrumentación de F0.3b: quedó acotado a <2.5% (IC95) en vez de resolverse.

### 3.4 La plataforma de medición tiene límites

i7-14700HX: **núcleos híbridos P/E**, portátil (throttling térmico), y topología
virtualizada (sin acceso a cpufreq ni probablemente al PMU).

Mitigaciones aplicadas: corridas intercaladas entre benchmarks (validado, sin
deriva térmica), cooldown de 20 s por defecto (bajarlo a 5 s subió la rsd de B2 de
3.6% a 6.5%), dispersión reportada como desviación estándar relativa y no como
rango.

Escalera si la varianza sigue estorbando después de F0.7:
1. `taskset` a un subconjunto homogéneo de CPUs
2. Mediciones de nodo único en un CCX de Hetzner (núcleos homogéneos, sin
   throttling, ~0.10 €/h)

### 3.5 La resolución define qué estás midiendo

Con `tile_size = 128`, una escena a 640 px da 15 tiles para 24 hilos: la mitad de
los núcleos ociosos y el wall time definido por el tile más lento. Medido: 760% de
CPU en un render a 400×400.

A 1920 px son 135 tiles, y ahí `quick` predice `full` con fidelidad (×4.58 vs
×4.53 en el total de B2).

### 3.6 Lo que dejó el roofline, y a dónde apunta

Cerrando F0.12 con los techos medidos de `gen1`:

| | B1 | B2 |
|---|---|---|
| intensidad aritmética | 0.68 FLOP/byte | 0.50 |
| working set | 1.3 KiB | 36.7 KiB |
| techo que aplica | banda de caché, 445 GiB/s | 443 GiB/s |
| % de ese techo | 28% | **40%** |
| % del pico de cómputo | 7.5% | 7.8% |

Las dos escenas caen a la **izquierda del quiebre** (1.13 FLOP/byte), o sea en la
región limitada por memoria, y muy por encima de la diagonal de DRAM: el árbol
vive en caché, como predecía §3.2.

Tres consecuencias para el plan:

**El margen que queda en recorrido no está en FLOPs.** Con 7.5% del pico de
cómputo y 40% del techo de banda, más aritmética vectorizada no compra nada. Lo
que queda es acortar la cadena de cargas dependientes, y eso es el **BVH ancho**
(F2.4b) y el trazado por paquetes — no SAH ni más SIMD escalar.

**El codo de escalado está en 8 hilos, y no es del código.** El barrido de
`ceilings` da 100% de escalado ideal a 4 hilos, 79% a 8 y **41% a 24**: se acaban
las 8 P-cores y entran las E-cores, que aportan la mitad por hilo. Cualquier
medición de escalabilidad fuerte en esta máquina va a tener ese codo, y hay que
caracterizarlo antes de F5.4 para no atribuírselo al scheduler.

**El margen grande está en el integrador, no en el recorrido.** Eficiencia
`1/(MSE·s)` medida: **B1 47.7 contra B2 930.6**, o sea que B1 necesita ~20× más
muestras para el mismo error. Eso no lo toca ninguna optimización de BVH — es
luz pequeña en caja cerrada, y es exactamente el blanco de **NEE (F1.3)** y
**MIS (F1.4)**. Ese 47.7 es la línea base de la Fase 1.

---

## 4. Reglas de higiene de medición

1. **Escenas inmutables.** Otros parámetros = carpeta nueva, nunca editar una
   existente. Nunca cambiar una escena que ya tiene mediciones.
2. `bench.toml` es la única fuente de verdad de la carga. `camera.json` guarda solo
   el encuadre; sus `image_width` y `samples_per_pixel` se sobreescriben siempre.
3. Todo resultado carga SHA de git, flag de dirty y hashes de escena.
4. Mediana + desviación estándar relativa + `n`. **Nunca rango min–max**: el rango
   de 3 muestras es sistemáticamente menor que el de 5.
5. Warmup descartado, cooldown entre repeticiones, corridas intercaladas al
   comparar variantes.
6. `wall` **y** `rays/s` **y** `samples/s` **y** (cuando aplique) MSE. Rays/s sola
   miente cuando cambia el integrador; samples/s sola miente cuando la imagen queda
   más ruidosa.
7. **Esquema aditivo.** `rt-bench` y `bench_sweep.py` escriben en el mismo
   `history.jsonl`: campos nuevos sí, renombrados nunca.
8. `standalone.rs` está congelado en su stdout (`"Procesado en {} ms"`): la barrida
   histórica lo parsea en 6 commits. La firma puede cambiar; esa línea no.

---

## 5. Fases

Los números son **identidad**, no orden. El orden de ejecución está en §6.

### Fase 0 — Cimientos

| | Actividad |
|---|---|
| F0.1 ✅ | `[profile.release]` con `lto="fat"`, `codegen-units=1`; `.cargo/config.toml` con `target-cpu=native` |
| F0.2 ✅ | Framebuffer `Vec<Vec4>` lineal; tone mapping como etapa de lectura; export EXR |
| F0.3a ✅ | `rt-bench`: manifiesto, wall time, metadata, salida JSONL |
| F0.3b ✅ | `RayStats` por `&mut`, rayos/s, samples/s, resumen de tiles |
| F0.3c ✅ | `rt-bench reference` a 25k spp / `max_depth` 100 → EXR, más `--reference` en `run`: MSE, MSE relativo y eficiencia `1/(MSE·s)` en espacio lineal. Adelantado respecto al plan porque F0.9 no era medible sin él |
| F0.4 ✅ | Determinismo: RNG sembrado por (píxel, sample, rebote); misma imagen con 1 hilo y con 24; test de regresión con hash |
| F0.5 ✅ | `thiserror` en librerías, `anyhow` en binarios; eliminar `unwrap`/`expect` de rutas no-inicialización |
| F0.6 ✅ | `Material` y `Hittable` de `dyn` a enums. Justificación medible, no estética |
| F0.7 ✅ | **BVH**: split por eje de mayor extensión (elimina `fastrand`), quitar el duplicado de `span==1`, orden de recorrido front-to-back |
| F0.8 ✅ | `Aabb{min,max: Vec3A}`, `inv_dir` precomputado en `Ray`, slab test SIMD, `Ray` por referencia. **+ BVH plano** (`Vec<FlatNode>` con índices `u32`, hojas multi-primitiva, primitivas contiguas) |
| F0.9 ✅ | Ruleta rusa (`MIN_BOUNCES`, techo del clamp en 1.0); `tile_size` 32 y `max_depth` 64 como defaults; `random_unit_vector` analítico —por F1.5, que necesita un mapa biyectivo de `[0,1)²` a la esfera, no por velocidad—; fix de `emitted(…)` → `(0.0, 0.0, rec.p)` hasta que F2.7 traiga UVs. **Descartados**: corte por atenuación (la ruleta hace lo mismo sin sesgo; quedó como el piso `0.05` del clamp) y RNG por tile (F0.4 ya lo dejó por (píxel, sample); volver atrás rompería el determinismo) |
| F0.10 ✅ | `rt-bench preview`: baja resolución con `NormalTracer`, metadata de procedencia en chunks tEXt del PNG. **+ `--tracer normal\|path` en `run`**, que da un benchmark de recorrido sin varianza de integrador — aunque solo de rayos primarios coherentes, que es el caso fácil |
| F0.11 ✅ | White furnace test automatizado. Tres casos, porque albedo 1.0 solo no alcanza: **1 es punto fijo de la multiplicación**, así que aplicar el albedo cero, una o tres veces da lo mismo. Con albedo 0.5 la esfera tiene que valer exactamente la mitad del fondo, y todo píxel de borde tiene que caer dentro del rango |
| F0.12 ✅ | `rt-bench ceilings` mide los techos de la máquina (pico FLOP/s por ancho SIMD y ancho de banda por nivel de caché, barriendo hilos) y los guarda por generación de hardware; `scripts/plot_roofline.py` los cruza con los contadores de F0.8. FLOPs y bytes contados analíticamente, sin PMU, para que la VM y un servidor queden comparables — el modelo está publicado en `cli_guide.md`. Los bytes los assertan los tipos vía `test_layout.rs`, no las reglas de alineación. Resultado en §3.6 |

### Fase 1 — Transporte de luz

El salto de calidad más grande de todo el plan.

**El blanco está medido.** Eficiencia `1/(MSE·s)`: **B1 47.7 contra B2 930.6**, o
sea que B1 necesita ~20× más muestras para el mismo error. No es geometría ni
recorrido — es luz pequeña en caja cerrada, donde los caminos encuentran la
fuente por azar. Cerrar ese factor es el trabajo de esta fase.

Toda la Fase 0 bajó el costo por muestra. F1 baja **cuántas muestras hacen
falta**, que es una mejora que el reloj no puede ver.

| | Actividad | Estado |
|---|---|---|
| F1.0 | **El instrumento antes del cambio**: `rt-bench converge` barre spp y `scripts/plot_convergence.py` grafica MSE contra tiempo. Sin esto nada de F1 es medible, igual que F0.3c tuvo que adelantarse para poder medir la ruleta rusa | ✅ |
| F1.1 | `Integrator` como trait/enum separado del recorrido | |
| F1.2 | Muestreo de BSDF: coseno-ponderado para difuso, **GGX VNDF** para microfacetas. Cada BSDF expone `sample()`, `eval()`, `pdf()`. Acá el furnace test de F0.11 pasa de trivial a crítico: hoy el lambertiano tiene el coseno implícito en `normal + random_unit_vector` y no puede equivocarse; con la pdf explícita sí | |
| F1.3 | **NEE** — muestreo directo de luces, con rayo de sombra `any-hit`. El BVH solo tiene `hit` (más cercano); hace falta una segunda función de recorrido que corte en el primer impacto, y vale medirla aparte | |
| F1.4 | **MIS** — heurística de potencia de Veach. F1.3 y F1.4 son el 80% del valor de la fase | |
| F1.5 | Muestreadores: estratificado, Halton, **Sobol con scrambling de Owen**, blue noise. Habilitado por el `random_unit_vector` analítico de F0.9: sin mapa biyectivo de `[0,1)²` a la esfera, un rechazo destruye la secuencia de baja discrepancia | |
| F1.6 | Estructura de luces: uniforme → por potencia → BVH de luces. **Pide una escena nueva**: con una sola luz (B1) no cambia nada | |
| F1.7 | **HDRI de entorno** con importance sampling (CDF 2D). Es lo que vuelve NEE útil en exteriores, o sea lo que arregla el caso de B2 | |
| F1.8 | PBR completo: GGX/Trowbridge-Reitz + Smith + compensación de energía multi-scatter; conductores con Fresnel complejo. Puerta del hito de la mesa | |
| F1.9 | Clamping de fireflies, **reportando el sesgo introducido**. La segunda mitad es el ítem: el clamping es sesgado y acá se cuantifica en vez de declararse aceptable, que es para lo que existe la infraestructura de referencia | |

**Orden propuesto:** F1.0 (hecho) → F1.1 → F1.2 → F1.3 → F1.4 → F1.5 → F1.9 →
F1.7 → F1.6 → F1.8. Misma lógica que en F0: primero el instrumento, después el
cambio.

**Dos cosas que hay que tener presentes al entrar:**

*Las referencias se van a invalidar.* F1.2, F1.3, F1.4 y F1.8 mueven la imagen
convergida, así que `run --reference` va a fallar con "la referencia es obsoleta"
— y eso es el guard funcionando. Hay que regenerarlas después de F1.4 y otra vez
después de F1.8, una hora de máquina cada vez.

*B2 puede empeorar con NEE.* Su fondo es un cielo grande que NEE no muestrea, así
que paga el rayo de sombra sin beneficio. Es la misma asimetría de la ruleta rusa
(§3.6 y LEARNED_LESSONS), y hay que medirla en las dos escenas antes de concluir.

**Riesgo de cronograma:** es la fase con más matemática y nueve ítems. Si algo se
come el calendario es esta. El bloque F1.3 + F1.4 no se negocia; F1.5 a F1.8 se
pueden recortar sin perder la narrativa.

### Fase 2 — Geometría y assets

| | Actividad |
|---|---|
| F2.1 | Import de mallas: OBJ + PLY (+ glTF). Escenas estándar: Sponza, San Miguel, Hairball |
| F2.2 | Intersección de triángulos (Möller–Trumbore o Woop), layout contiguo sin `Arc` por primitiva |
| F2.3 | *(promovido a F0.8)* BVH plano |
| F2.4 | Construcción SAH binned, reemplazando el split por mediana |
| F2.5 | **Construcción paralela**: LBVH con códigos Morton + radix sort paralelo, o SAH binned con Rayon |
| F2.5b | Primitiva caja vía slab (reutiliza F0.8) + `Transform` con `Affine3A`. *(era F0.13; movida acá porque no bloquea nada de la Fase 1 y es el prerequisito directo de F2.6)*. Al llegar acá, revisar el test ignorado del rayo coplanar: la caja llena las escenas de superficies alineadas a ejes, así que el `0 * inf = NaN` pasa de improbable a plausible. **No aplicar a B1**: cambiar esa escena rompe la serie histórica |
| F2.6 | Instancing + BVH de dos niveles (TLAS/BLAS) |
| F2.7 | UV mapping en esferas, quads y triángulos; derivadas para filtrado |
| F2.8 | Texturas procedurales: Perlin, Simplex, Worley/Voronoi, checker, gradientes, turbulencia, **domain warping**. Grafo componible |
| F2.9 | Texturas de imagen: carga, filtrado bilineal, mipmaps |
| F2.10 | Normal + bump maps en espacio tangente |
| F2.11 | Parallax occlusion mapping |
| F2.12 | Displacement (opcional; considerar micro-displacement en intersección) |
| F2.13 | Cuádricas (paraboloides, elipsoides, conos) + CSG con unión/intersección/**diferencia** |
| F2.14 | SDFs + sphere marching como un `Hittable` más → CSG trivial y **Mandelbulb** |
| F2.15 | Motion blur: dimensión temporal en el rayo, AABBs con movimiento |
| F2.16 | `PUT /display` + resync completo del framebuffer |

### Fase 3 — Materiales, volúmenes y cámara

| | Actividad |
|---|---|
| F3.1 | **Absorción Beer-Lambert** en dieléctricos. Es lo que produce el efecto "río de lava" |
| F3.2 | Medios homogéneos: scattering isotrópico + Henyey-Greenstein. Niebla |
| F3.3 | Medios heterogéneos: delta/ratio tracking, import de **OpenVDB** |
| F3.4 | Volúmenes emisivos; muestreo equiangular para god rays |
| F3.5 | Materiales por capas: clearcoat sobre base |
| F3.6 | Anisotropía (metal cepillado) |
| F3.7 | **SSS por random walk**: piel, mármol, cera, jade |
| F3.8 | **Sistemas de lentes reales** (Kolb et al.): bokeh, distorsión, viñeteo y aberración cromática caen de la física |
| F3.9 | Formas de apertura: palas del diafragma, cat's eye |
| F3.10 | **AOVs**: albedo, normal, profundidad, directo/indirecto, ID de objeto |
| F3.11 | **Denoising**: À-Trous guiado por AOVs, o integrar Intel OIDN |

**🎯 Hito: la mesa de epoxi.** Vetas por F2.8, borde vivo por función procedural
3D, resina por F3.1 (+ F3.4 opcional), barniz por F3.5, luz por F1.7, DOF por F3.8.
Las cáusticas nítidas esperan a F6.1 — un path tracer unidireccional converge a
ellas pero lento. Documentar la limitación y comparar ambas versiones después.

### Fase 4 — Plataforma

| | Actividad |
|---|---|
| F4.1 | Modelo de Job: **spec inmutable** + estado mutable separado; hash de contenido → dedup y caché |
| F4.2 | Máquina de estados explícita: `Queued → Running → {Completed, Failed, Cancelled}` + `Paused` |
| F4.3 | Cola + CRUD + listado, con paginación, filtros e **idempotency keys** |
| F4.4 | **Render progresivo**: refinamientos sucesivos (1, 2, 4, 8… spp) |
| F4.5 | **Checkpointing**: serializar buffer de acumulación + estado del muestreador |
| F4.6 | Persistencia: EXR canónico + PNG derivado, con metadata del render |
| F4.7 | **API de Assets** direccionada por contenido (hash): texturas, mallas, HDRIs, VDBs |
| F4.7b | base64 en el DTO del stream + troceado del evento inicial en patches. *(era F0.14; movida acá porque es plomería de streaming y su lugar natural es junto a F4.8)* |
| F4.8 | **Streaming v2**: canal SSE por job, reconexión con `Last-Event-ID`, backpressure, keep-alive, binario sin base64 |
| F4.9 | Prioridad y preemption: los previews interrumpen batch |
| F4.10 | **Métricas** Prometheus: rayos/s, samples/s, histograma por tile, utilización, profundidad de cola |
| F4.11 | Frontend: lista de jobs, preview en vivo, comparador de imágenes |
| F4.12 | `GET /render/hdr` (endpoint, no streaming) + canal de stream por job |

**Pendientes de deuda ya identificados** que caen acá: `is_finished: bool` es
insuficiente (F4.2 lo reemplaza), el broadcast global sirve a todos los jobs
(F4.8), `Lagged` pierde tiles sin recuperación (resync completo), el tile mágico
`999999` debería ser un evento SSE nombrado.

### Fase 5 — Distribución

| | Actividad |
|---|---|
| F5.1 | Abstracción `Transport` + implementación gRPC |
| F5.2 | Protocolo de worker: registro, heartbeat, pull de trabajo, entrega, assets por hash con caché local |
| F5.3 | Particionamiento **por muestras** vs **por tiles**. Implementar ambos y comparar — ese es el resultado |
| F5.4 | Schedulers: estático, dinámico (pull), **work-stealing**. Estudio de granularidad |
| F5.5 | Tolerancia a fallos: worker muere a mitad de job → reasignación sin perder muestras completadas |
| F5.6 | Contenedores + K8s: imagen distroless, deployment de workers, coordinador |
| F5.7 | Terraform con **destrucción** automatizada (ver §8) |
| F5.8 | CI/CD: tests, clippy, benchmarks de regresión por PR, build de imágenes |

⚠️ **El u8 no se puede filtrar al protocolo worker→coordinador.** Combinar muestras
entre nodos exige lineal HDR; promediar u8 es matemáticamente incorrecto. Son dos
cables con requisitos opuestos.

### Fase 6 — Avanzado + reporte

| | Actividad | Prioridad |
|---|---|---|
| F6.1 | Photon mapping / progressive PM → cáusticas reales de la mesa | Alta |
| F6.2 | Espectral (hero wavelength) + dispersión + películas delgadas | Alta |
| F6.3 | ReSTIR DI | Media |
| F6.4 | VCM / BDPT | Media |
| F6.5 | BSDF de pelo (Marschner) | Baja |
| F6.6 | Backend GPU (wgpu) para preview; si no alcanza, *future work* con diseño esbozado | Ver §7 |
| F6.7 | **El reporte.** 3 semanas reservadas, no comprimir | Máxima |

---

## 6. Orden de ejecución

El orden cambió respecto al plan original por la evidencia de §3.

Ejecutado, en este orden: **F0.7** (elimina el `fastrand` del BVH, que era el piso
de ruido de todas las mediciones), **F0.4**, **F0.5**, **F0.6**, **F0.8** (la
apuesta grande de §3.2: ×1.51 en B1 y ×1.57 en B2), **F0.3c** — adelantado
respecto al plan, porque sin referencia y MSE la ruleta rusa de F0.9 no era
medible — **F0.9**, **F0.10**, **F0.11** y **F0.12**.

F0.12 se hizo antes de F1 por oportunidad: los FLOPs por rayo se cuentan a mano
hoy, y después de que F1 meta NEE, MIS y GGX eso deja de ser viable. El código
nunca va a ser más simple de analizar.

F0.13 (caja + `Transform`) se movió a **F2.5b**, justo antes de instancing, que es
lo que la necesita. F0.14 (base64 en el stream) se movió a **F4.7b**, junto al
resto del streaming. Ninguna bloqueaba la Fase 1 y tenerlas en la Fase 0 solo
retrasaba el salto de calidad.

**Lo que sigue: Fase 1 → 2 → 3 (hito de la mesa) → 4 → 5 → 6.**

Y con la Fase 0 cerrada cambia la métrica: de acá en adelante es la eficiencia
`1/(MSE·s)` de §10, no el reloj. La ruleta rusa dejó B2 10% más rápido y 26%
peor, y el cronómetro solo veía la primera mitad.

---

## 7. Huecos conocidos del perfil

Lo que un evaluador va a señalar, y el plan para cada uno.

| Hueco | Plan |
|---|---|
| **Cero aceleradores** | El mayor. F6.6 con wgpu, aunque sea limitado a rayos primarios, con comparación CPU vs GPU medida. Convierte "nunca toqué una GPU" en "tengo una opinión informada, con números". Compite en tiempo con F6.1/F6.2 — decisión consciente pendiente |
| **Sin MPI ni cluster real** | Defendible por presupuesto. Documentar como limitación |
| **Carga embarrassingly parallel** | Mitigado por F5.3/F5.4/F5.5. Y dos oportunidades de acoplamiento fuerte que salen casi gratis: el **denoiser distribuido es un stencil** (halo exchange entre tiles en nodos distintos) y la **acumulación progresiva multinodo es una reducción colectiva** |
| **Sin producto de investigación** | Conseguir lectura externa del reporte; apuntar a arXiv, workshop estudiantil o póster |
| **Poco cómputo numérico** | No se cubre. Asumido |

---

## 8. Infraestructura y presupuesto

Techo: **50 USD** de gasto propio, más los créditos de GCP/AWS para una corrida
final grande.

Fases 0–4 completas: costo cero. La máquina local es la plataforma de nodo único.

Para F5, **Hetzner Cloud CCX** (vCPU dedicado, facturación por hora, ~0.10 €/h),
no dedicados de Robot (mensuales con cuota de instalación):

| Uso | Costo aprox. |
|---|---|
| Desarrollo de F5 (2 nodos × ~15 h) | ~€3 |
| Escalabilidad (8 nodos × 8 vCPU × 6 h) | ~€5 |
| Render final del paper (8 nodos × 4 h) | ~€3 |
| Margen | ~€15 |

**Reglas para no quemar el presupuesto** — la forma número uno de gastarlo es
olvidar un cluster prendido:

1. `terraform destroy` en el mismo script que el `apply`. Nunca provisionar a mano.
2. TTL duro: cronjob que apaga todo pasadas N horas.
3. Destruir **volúmenes y snapshots** también, no solo instancias.
4. Alerta de gasto desde el día uno.
5. Misma región y mismo tipo de instancia entre corridas, o las comparaciones no
   valen.

Ese requisito de teardown automático es la mejor justificación de Terraform en el
proyecto: no es DevOps decorativo, es control de presupuesto y reproducibilidad.

---

## 9. Suite de benchmarks

Cada escena aísla un eje distinto. Si todas estresan lo mismo, la suite no dice
nada.

| | Nombre | Aísla | Estado |
|---|---|---|---|
| **B1** | `cornell-glass` | **Transporte de luz.** 19 objetos, luz pequeña, caja cerrada, dieléctrico. El BVH es irrelevante; el número refleja integrador y muestreador | ✅ |
| **B2** | `rtow-spheres` | **Geometría y rayos incoherentes.** 521 esferas, DOF. Conecta con el historial de commits — no debe cambiar nunca | ✅ |
| — | furnace | Correctitud con verdad analítica. No cuenta contra las 5 | F0.11 |
| **B3** | Sponza (~260k tri) | BVH y mallas reales | F2 |
| **B4** | Hairball (~2.8M) / San Miguel (~10M) | Presión de memoria y peor caso | F2 |
| **B5** | Nube VDB | Medios heterogéneos; costo por rayo altísimo y variable | F3 |

Agregar un benchmark es `mkdir scenes/bench/<name>/` con `bench.toml`,
`scene.json` y `camera.json`. Ambos drivers lo descubren por glob.

---

## 10. Plan de medición por fase

| Fase | Métrica primaria | Escena | Figura |
|---|---|---|---|
| F0.3 | rayos/s y samples/s de baseline | B1, B2 | tabla de baseline |
| F0.4 | hash idéntico con 1 hilo y con 24 | B1, B2 | test |
| F0.6 | Δ rayos/s | B2 | barra de contribución por optimización |
| F0.7–F0.8 | rayos/s + **nodos visitados/rayo** | B2 | idem + calidad de árbol |
| F0.9 | **eficiencia `1/(MSE·s)`** contra referencia. El reloj solo no sirve: la ruleta rusa dejó B2 10% más rápido y 26% peor | B1, B2 | eficiencia por commit |
| F0.10 | Mray/s y nodos/rayo con `--tracer normal`: recorrido sin varianza de integrador | B1, B2 | tabla de recorrido puro |
| F0.11 | furnace: albedo 1.0 invisible, albedo 0.5 exactamente a la mitad | furnace | test |
| F0.12 | intensidad aritmética y % del techo que aplica; el % del pico de cómputo es secundario porque no sobrevive un error de conteo | B1, B2 | roofline con techos por generación |
| **F1** | **MSE vs tiempo** a igual tiempo, no a igual muestras (`rt-bench converge`). Se lee sobre diagonales de eficiencia constante; la pendiente −1 es el ideal insesgado y una cola plana delata un piso de sesgo | **B1**, B2 | convergencia por muestreador × integrador |
| F2 | tiempo de build vs #triángulos, memoria/triángulo | B3, B4 | escalabilidad del build paralelo |
| F3 | costo relativo por feature | B5 | tabla de costo |
| F5 | escalabilidad fuerte y débil, desbalanceo, tiempo de recuperación | B3/B4 | curvas de eficiencia |

**Métricas ya instrumentadas:** `rays`, `rays_per_sec`, `samples`,
`samples_per_sec`, `build_ms`, y el resumen de tiles (`min`, `median`, `p95`,
`max`, `imbalance`).

**Pendientes:** `node_visits` y `prim_tests` requieren pasar el contexto dentro de
`Hittable::hit()` — llegan con F0.8, para no tocar cada `hit()` dos veces.
`image_hash` con F0.4, `mse` con F0.3c.

---

## 11. Herramientas

| | Qué hace |
|---|---|
| `rt-bench run` | Mide el árbol actual, de HEAD en adelante. Ver `crates/rt-bench/cli_guide.md` |
| `rt-bench reference` | Renderiza el ground truth a spp alto → EXR + sidecar con hashes |
| `rt-bench preview` | Previews de baja resolución con procedencia en chunks tEXt del PNG |
| `rt-bench ceilings` | Techos de la máquina por generación. También sirve de sonda de calibración: correrlo antes de una barrida dice si la máquina está en el estado que crees |
| `rt-bench converge` | Barrido de spp para la curva MSE vs tiempo |
| `scripts/bench_sweep.py` | Barrida histórica sobre commits pasados, desde fuera del código medido |
| `scripts/tile_sweep.py` | Barrido de `tile_size` con verificación de invariantes |
| `scripts/gen_cornell.py` | Genera B1 y verifica que la geometría no se solape |
| `scripts/plot_evolution.py` | Tiempo por commit |
| `scripts/plot_benchmarks.py` | B1 y B2 normalizados en los mismos ejes |
| `scripts/plot_hardware.py` | Una serie por generación de hardware, para leer el corte |
| `scripts/plot_roofline.py` | Intensidad aritmética contra los techos |
| `scripts/plot_convergence.py` | MSE contra tiempo sobre diagonales de eficiencia |

Todos los scripts de gráficas imprimen la ruta del PNG por stdout y las notas por
stderr, así que `| xargs kitten icat` funciona en todos.

| Dataset | |
|---|---|
| `bench/history.jsonl` | Compartido por `rt-bench run` y el sweep. **Commitear siempre** |
| `bench/hardware.toml` | Generación activa. Actualizar **antes** de medir en máquina nueva |
| `bench/ceilings/*.json` | Techos por generación |
| `bench/convergence.jsonl` | Curvas de convergencia por hardware y commit |
| `bench/reference/*.exr` | Ground truth, ignorado por tamaño; los sidecars `.json` sí se commitean |

Instalar cuando se llegue a perfilado: `samply` (`cargo install samply`, sin root,
flamegraph en el navegador) y `perf` vía `linux-tools`. El PMU **no** está
accesible en la VM — de ahí que F0.12 cuente FLOPs y bytes analíticamente, que
además es lo que mantiene comparables la VM y un servidor dedicado.

---

## 12. Decisiones abiertas

1. **GPU vs gráficos avanzados en F6.** Para admisiones a HPC la GPU vale más que
   photon mapping y espectral. Son los que más ilusión hacen. Decidir
   conscientemente, no dejar que se escape.
2. **Plataforma de medición.** Si tras F0.7 la varianza sigue alta, mover las
   mediciones de nodo único a Hetzner.
3. **Framework del frontend (F4.11).** Recomendación: híbrido — UI en Svelte,
   pipeline de píxeles (decode HDR + tone mapping + exposición) como módulo
   Rust→WASM compartido con el renderer. Da el slider de exposición a 60fps y
   garantiza que el PNG del servidor y lo que ve el navegador salgan del mismo
   código.
