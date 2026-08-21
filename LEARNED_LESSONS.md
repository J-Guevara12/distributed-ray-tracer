# Lecciones aprendidas

Errores que ya cometimos y hallazgos que costaron caro. Si algo del código o del
plan parece arbitrario, la razón probablemente está acá.

---

## Medición

### `rt-bench` no compila; `bench_sweep.py` sí

`rt-bench` mide el binario que está en `target/release/`, no el árbol de trabajo.
El driver de la barrida es el que crea un worktree y compila cada commit.

Confundirlos costó una tanda completa de mediciones de F0.7: los números tenían
la etiqueta nueva y el código viejo. El ciclo es siempre
**implementar → commitear → `cargo build --release` → medir**.

### Una medición contaminada lleva a una conclusión invertida

Al diagnosticar la regresión de F0.7 medimos una variante con la máquina cargada
(rsd 28.8%, justo después de un build) y concluimos que la clave de ordenamiento
no era la causa. Era la causa **entera**: 4087 ms contra 2492 ms.

Si la dispersión de una corrida es alta, no se concluye nada de ella. Se repite.

### No medir con la máquina caliente ni cargada

Es un portátil con núcleos híbridos P/E y throttling térmico. Tras una hora de
builds y benchmarks, las dos escenas midieron ~1.6× peor que *cualquier*
variante — por debajo incluso de la versión rota. El síntoma es que `Mray/s`
cae por debajo de todo lo medido antes.

Cooldown de 20 s por defecto. Bajarlo a 5 s subió la rsd de B2 de 3.6% a 6.5%.

### El rango min-max no compara entre corridas con distinto `n`

El rango de 3 muestras es sistemáticamente menor que el de 5. Comparar un
"±31% de rango" con un "14% de rsd" es inválido, y por eso creímos un rato que
la varianza no había bajado cuando sí.

Usar desviación estándar relativa e imprimir siempre el `n`.

### Alternar variantes, no agrupar

Todas las repeticiones de A y después todas las de B hace que B parezca más
lento por termodinámica. El A/B que resolvió la regresión de F0.7 solo fue
concluyente al alternar los binarios en la misma sesión.

### `quick` y `full` no miden la misma mezcla

El mismo arreglo dio **+19% en `quick` (20 spp) y +6.6% en `full` (250 spp)**. A
más muestras los rayos secundarios incoherentes pesan más y la calidad del árbol
pesa relativamente menos.

`quick` sirve para detectar la dirección del cambio, no para predecir su
magnitud en `full`.

### El reloj miente cuando el cambio negocia ruido por tiempo

La ruleta rusa dejó B2 un 10% más rápido y **26% peor**. Las dos cosas son
ciertas: hace menos trabajo y produce una imagen proporcionalmente más ruidosa
que lo que ahorró. El cronómetro solo ve la mitad.

La métrica es `eficiencia = 1 / (MSE × segundos)`, medida contra una imagen de
referencia convergida. Un cambio que parte el tiempo y duplica el error es un
empate, y esa fórmula lo dice.

Desde F0.9 **ningún cambio de muestreo o de terminación se puede evaluar con el
reloj.** Vale para la ruleta rusa, para los muestreadores de F1.5, para NEE y
para el clamping de fireflies de F1.9.

### La eficiencia no depende del spp, y eso la vuelve la métrica correcta

Medido en las dos escenas, antes y después de la ruleta: `MSE ∝ 1/spp` con
error menor al 3% en los cuatro casos (B1 25.18 contra 25.00 esperado, B2 12.59
contra 12.50). Como el tiempo va como `spp`, el producto es constante y la
eficiencia sale igual en `quick` que en `full` (desvío 1.4–11%).

Consecuencia práctica: **se puede medir eficiencia en `quick` y el número vale
para `full`**, que es lo contrario de lo que pasa con el tiempo de pared.

Y el chequeo es gratis: si la MSE deja de escalar como `1/spp`, hay sesgo. Es la
forma de detectar que una referencia quedó obsoleta o que el integrador dejó de
ser insesgado.

### Una métrica mal definida apunta al revés

`TileSummary::imbalance` es `(max - media) / media`: cuánto se despega el peor
tile del promedio. Eso mide **heterogeneidad del contenido**, no hilos ociosos, y
sube con tiles chicos por construcción. En la barrida de `tile_size` marcaba a
256 px como el mejor repartido justo cuando era el más lento.

Lo que hay que medir es si sobran hilos sin trabajo:

    eficiencia = suma(tiempos de tile) / (hilos × tiempo de pared)

La suma se recupera del resumen que ya se guarda: `media = max / (1 + imbalance)`.
Antes de creerle a una métrica, escribir qué pasa en los dos extremos.

### `rays/s` no es una constante de hardware

Es por escena. Un "rayo" es una consulta de intersección, y su costo depende de
la profundidad del BVH, del tipo de primitiva y de si la escena cabe en caché.
B1 saca 53 Mray/s y B2 37 Mray/s con el mismo binario.

Solo es comparable **longitudinalmente dentro de una escena**. Y `samples/s` y
`rays/s` pueden moverse en direcciones opuestas: la ruleta rusa sube el primero
sin tocar el segundo, y NEE hace lo contrario.

---

## Rendimiento

### El renderer está limitado por latencia de memoria, no por cómputo

`lto="fat"` + `codegen-units=1` + `target-cpu=native` dieron **×1.00** medido en
6 commits. La causa probable: punteros `Arc<dyn Hittable>` dispersos en el heap
con una llamada virtual por nodo. Mejor codegen no ayuda a un bucle que espera
la caché.

Consecuencia: el BVH plano con primitivas contiguas vale más que optimizar la
aritmética del slab test.

### El BVH cuesta ~7% en escenas pequeñas

En B1 (19 objetos) el barrido lineal de `HittableList` gana al BVH: 22.5 s
contra 24.2 s, sin solaparse. Ese ×0.93 es el costo puro de la indirección.

### `longest_axis` sobre extent de centroides se rompe con outliers de escala

La esfera del piso de B2 (`r=1000`, centro `y=-1000`) infla el extent de Y a
1001 contra 21.87 de X y Z, así que el heurístico elige **Y en la raíz siempre**
— y Y es el eje que no separa nada, porque las 517 esferas pequeñas están todas
apoyadas en el suelo.

Es una estructura de escena muy común (piso, terreno, skybox), no una rareza.
**Sponza en F2 la tiene.**

### Ordenar por centroide en un eje degenerado es ordenar por otra cosa

Las esferas pequeñas de B2 cumplen `center.y ≈ radius`, así que ordenar por
centroide en Y **equivale a ordenar por radio**: intercala esferas de todo el
plano XZ y el corte por mediana queda espacialmente sin sentido. Cuesta 33%.

`bbox.min` funciona mejor **por accidente**: empata (45 valores distintos contra
473), el sort estable conserva el orden del archivo, y ese orden viene de los
bucles del generador, o sea que es espacialmente coherente. En B1 `bbox.min` es
de hecho 2% *peor* que centroide.

Ninguna clave de orden es buena universalmente. El arreglo de fondo es SAH
binned sobre posiciones de corte, que no depende de la clave.

### La cuantización de tiles sobre hilos domina cualquier efecto de caché

`tile_size` 128 → 32 dio **−12.9% en B1 y −9.0% en B2** sin tocar el renderer.
No es localidad: es que la última ronda de planificación corre a medias.

| B1 | tiles | rondas de 24 | util. predicha | eficiencia medida |
|---|---|---|---|---|
| 32 | 1024 | 43 | 99.2% | 98.5% |
| 128 | 64 | 3 | 88.9% | 83.5% |
| 256 | 16 | 1 | 66.7% | 54.9% |

Con 256 px hay 16 tiles para 24 hilos: ocho no tocan la escena. El modelo
`n / (ceil(n/hilos) × hilos)` predice las cinco mediciones de las dos escenas.

Y el efecto contrario existe pero pierde por goleada: con tiles de 256 px el
trabajo **total** de CPU baja 14% (293 s contra 342 s) por mejor localidad y
menos locks, y aun así el reloj sube 54%. Ese 14% es lo que dejaría sobre la
mesa un scheduler capaz de partir tiles a demanda — insumo para F5.4.

16 y 32 empatan dentro del ruido. Entre empatados gana el mayor: `write_tile`
toma un write-lock sobre el framebuffer entero, así que menos tiles es menos
contención.

### La ruleta rusa depende de la escena, y el throughput no alcanza para decidir

B1 ×1.58 de eficiencia, B2 ×0.75. La misma optimización, resultados opuestos.

La ruleta decide con el throughput acumulado, que es un proxy de la contribución
que falta. El proxy falla cuando lo que viene adelante es una fuente **grande y
garantizada**:

* **B2, cielo abierto**: contribución restante grande y de baja varianza — el
  camino *siempre* encuentra el cielo. Matarlo pierde algo seguro.
* **B1, caja cerrada**: contribución restante chica y de alta varianza — la luz
  es un quad diminuto que el camino casi nunca encuentra. Matarlo es gratis.

Con `MIN_BOUNCES` de 3 a 5 se recupera parte (B2 sube a ×0.91, B1 baja a ×1.40;
media geométrica ×1.13 contra ×1.09), pero es un compromiso, no una solución: en
B2 la MSE sigue subiendo más de lo que baja el tiempo. La respuesta de fondo es
NEE, que muestrea la luz directamente en vez de esperar a chocarla.

Corolario para más adelante: **la ruleta vale más en escenas cerradas y oscuras
que en abiertas y brillantes.** Los interiores de F2.1 (Sponza, San Miguel) son
el caso favorable; B2 es el adverso, no el típico.

### `max_depth` solo importa donde la varianza ya bajó

Subirlo de 15 a 64 con la ruleta puesta no cambió nada medible: mismo tiempo,
misma eficiencia, `ray/smp` 3.49 contra 3.54. Con la ruleta el tope nunca aplica.

La razón es aritmética. Truncar en 15 con albedo ~0.73 deja un sesgo de
`(0.73¹⁵)² ≈ 7.6e-5`, y la MSE por varianza a 8 spp es `8.3e-2`: **tres órdenes
de magnitud abajo, invisible.** A 25000 spp la varianza cae a ~2.7e-5 y el sesgo
pasa a ser comparable.

O sea que la profundidad alta hace falta **en la referencia, no en los
benchmarks**. En los benchmarks es red de seguridad contra caminos patológicos
(reflexión interna total en un dieléctrico), no una mejora medible.

### Contadores atómicos en el bucle caliente cuestan 30–40×

Un `fetch_add(1, Relaxed)` por nodo visitado, desde 24 hilos sobre la misma
línea de caché, llevó B2 de 3 s a **114 s**. Fueron ~1.9 mil millones de
incrementos contendiendo.

Los contadores van locales por hilo y se fusionan al final, como hace
`RayStats`. Nunca atómicos compartidos.

### El no determinismo aparece como ruido de medición Y como tests intermitentes

`BvhNode` elegía el eje de corte con `fastrand`, sembrado por proceso, así que
cada corrida construía un árbol distinto. Se manifestó en dos lados:

* dispersión de 7–9% que impedía atribuir cambios menores;
* `test_bvh_traversal_hit_and_short_circuit` fallando ~40% de las corridas.

Al volverlo determinista, la dispersión cayó a **2.0–2.3%**.

---

## Higiene de datos

### El SHA es el único campo verificable

`commit_label` lo escribe el usuario y nadie lo comprueba. Por eso dos estados
de código no pueden compartir commit: medir dos variantes con `--allow-dirty` y
el mismo SHA deja el dataset sin forma de distinguirlas.

### Editar un `bench.toml` cambia su hash y rompe el agrupamiento

Aunque el cambio sea solo comentarios. Por eso el script de gráficas agrupa por
la **carga real** (`scene_hash`, `camera_hash`, `width`, `spp`) y no por el hash
del manifiesto, con una tabla histórica para los registros anteriores a que
`width`/`spp` estuvieran en el registro.

### El hash de imagen no es una identidad estable

Dos aceleradores correctos pueden dar imágenes distintas. En B1 el BVH plano
cambió 13 píxeles de 1 048 576 respecto del árbol anterior, y el hash de `full`
se movió mientras el de `quick` no.

La causa es geométrica: donde dos paredes perpendiculares comparten arista, un
rayo que da justo en la arista **intersecta los dos quads con el mismo `t`
exacto**. `PlanarShape::hit` usa `contains` (inclusivo), así que gana el que se
pruebe último, y eso lo decide el orden de recorrido. Las esferas usan
`surrounds` (exclusivo) y rechazan el empate — por eso B2 nunca se movió.

Los píxeles afectados caen todos sobre la diagonal de la imagen porque la cámara
de B1 está en `(278, 278, -800)`, simétrica sobre una caja de `555³`: la arista
`(0, 0, z)` se proyecta desde una esquina hasta el punto de fuga central.

No hay nada que arreglar. Ninguno de los dos desempates es correcto y las
paredes adyacentes tienen que compartir arista. Pero el hash va a seguir
moviéndose cada vez que se toque el recorrido, así que **no sirve como reja de
regresión**. El invariante que sí sirve está en `test_bvh_matches_linear_scan`.

### `quick` no alcanza como reja de regresión de imagen

El cambio de arriba lo detectó `full` (200 spp) y no `quick` (8 spp). Todas las
verificaciones de "hash intacto" de F0.6 y F0.8 se habían hecho solo con `quick`.

Con la semilla por `(píxel, sample)`, las muestras `0..7` son los mismos rayos a
8 spp que a 200 spp, así que `quick` es un subconjunto estricto de `full`: puede
confirmar que algo cambió, nunca que nada cambió.

### Las escenas de benchmark son inmutables

`bench.toml` es la única fuente de verdad de la carga; `camera.json` guarda solo
el encuadre y sus `image_width`/`samples_per_pixel` se sobreescriben siempre.
Para otros parámetros, carpeta nueva.

Ya pasó: el `quick` de B2 fue 640/64 antes del 2026-08-13 y 1920/20 después, y
sin `width`/`spp` en el registro los dos grupos eran indistinguibles.

---

## Tests

### El invariante de un acelerador es igualar al barrido lineal

Un BVH es una optimización, así que tiene que devolver exactamente lo mismo que
revisar todas las primitivas. Eso es verificable y no depende de ninguna escena
de benchmark: `test_bvh_matches_linear_scan` dispara rayos aleatorios contra
`LinearScan` y compara `t`, material y normal bit a bit.

Es lo que convirtió una sospecha en un diagnóstico. Con 20 millones de rayos y
cero discrepancias quedó descartado que el BVH estuviera mal, y las 398
discrepancias que sí aparecieron con los rayos primarios señalaron la causa real
en una corrida.

### Una hipótesis plausible no es un diagnóstico

Ante los 13 píxeles de B1 la explicación obvia era que las bases de las cajas del
Cornell box son coplanares con el piso — lo son, se verifica en el JSON de la
escena. Quitarlas no cambió **ni un píxel**: la causa eran las aristas de la
caja, otra coincidencia distinta.

El costo de comprobar era una corrida de 10 segundos. Comprobar siempre la
hipótesis antes de escribirla en el reporte, sobre todo cuando encaja demasiado
bien.

### Un test que pasa por suerte de ordenamiento es peor que ningún test

`MockHittable` devolvía un `t` fijo de `1.0` sin importar la distancia. El BVH
recorta el intervalo del hijo derecho con el `t` del izquierdo, así que con un
`t` constante el árbol descartaba impactos válidos según qué eje aleatorio le
tocara — y el test solo pasaba cuando el orden salía favorable.

Un mock tiene que respetar los invariantes que el código bajo prueba usa. Aquí,
derivar `t` del punto de impacto.
