<div align="center">

# Railroad Tycoon II (rrt2-decomp) — Guía del Jugador

[English](player-guide.md) · [Português](player-guide.pt-BR.md) · **Español** · [Deutsch](player-guide.de.md) · [中文](player-guide.zh-CN.md) · [Русский](player-guide.ru.md)

</div>

---

Bienvenido a **rrt2-decomp**, una reimplementación moderna y de código abierto del clásico de estrategia de 1998 *Railroad Tycoon II Platinum*, escrita desde cero en Rust. Esta guía detalla todo lo que necesitas saber: instalación y puesta en marcha, conceptos esenciales de la gestión ferroviaria, atajos de control, mejoras técnicas respecto al juego original de 1998 y características modernas opcionales.

> [!IMPORTANT]
> **rrt2-decomp** **no incluye ningún recurso propietario del juego**. Para jugar, necesitas disponer de una copia legítima de *Railroad Tycoon II Platinum* (por ejemplo, de [GOG](https://www.gog.com/game/railroad_tycoon_2_platinum), Steam o una instalación original en CD-ROM). El motor lee los archivos gráficos, de audio, mapas y textos directamente de tu instalación existente en modo de solo lectura, siguiendo el mismo modelo de proyectos como OpenRCT2 u OpenTTD.

---

## Índice de Contenidos

1. [Instalación y Puesta en Marcha](#1-instalación-y-puesta-en-marcha)
   - [Requisitos del Sistema y Archivos](#requisitos-del-sistema-y-archivos)
   - [Detección Automática de Instalaciones](#detección-automática-de-instalaciones)
   - [Configuración Manual de la Ruta](#configuración-manual-de-la-ruta)
   - [Paquetes por Sistema Operativo](#paquetes-por-sistema-operativo)
   - [Directorios de Configuración y Partidas Guardadas](#directorios-de-configuración-y-partidas-guardadas)
2. [Primeros Pasos](#2-primeros-pasos)
   - [Modos de Juego: Campañas, Escenarios y Sandbox](#modos-de-juego-campañas-escenarios-y-sandbox)
   - [Fundar tu Compañía Ferroviaria](#fundar-tu-compañía-ferroviaria)
   - [Construcción de Vías Férreas](#construcción-de-vías-férreas)
   - [Estaciones y Mejoras de Infraestructura](#estaciones-y-mejoras-de-infraestructura)
   - [Compra de Locomotoras](#compra-de-locomotoras)
   - [Montaje de Composiciones y Rutas](#montaje-de-composiciones-y-rutas)
   - [Gestión Financiera y Mercado Bursátil](#gestión-financiera-y-mercado-bursátil)
3. [Controles y Atajos de Teclado](#3-controles-y-atajos-de-teclado)
   - [Navegación de Cámara y Zoom](#navegación-de-cámara-y-zoom)
   - [Velocidad de Simulación y Pausa](#velocidad-de-simulación-y-pausa)
   - [Paneles de Información y Acceso Rápido](#paneles-de-información-y-acceso-rápido)
   - [Informes Financieros y Gráficos Interactivos](#informes-financieros-y-gráficos-interactivos)
   - [Historial de Eventos y Notificaciones](#historial-de-eventos-y-notificaciones)
   - [Soporte para Mando y Steam Deck](#soporte-para-mando-y-steam-deck)
4. [Diferencias Respecto al Original de 1998](#4-diferencias-respecto-al-original-de-1998)
   - [Simulación Bit a Bit Fiel y Determinista](#simulación-bit-a-bit-fiel-y-determinista)
   - [Pantallas Modernas, Panorámicas y 4K](#pantallas-modernas-panorámicas-y-4k)
   - [Tipografía Vectorial Nítida y Soporte HiDPI](#tipografía-vectorial-nítida-y-soporte-hidpi)
   - [Multijugador Lockstep Moderno por TCP/IP](#multijugador-lockstep-moderno-por-tcpip)
   - [Localización Dinámica en Seis Idiomas](#localización-dinámica-en-seis-idiomas)
5. [Características Modernas Opcionales](#5-características-modernas-opcionales)
   - [Correcciones de Errores Originales por Regla (`Fix`)](#correcciones-de-errores-originales-por-regla-fix)
   - [Deshacer Construcción (`Ctrl+Z`)](#deshacer-construcción-ctrlz)
   - [Gestor de Flota y Reemplazo Masivo](#gestor-de-flota-y-reemplazo-masivo)
   - [Superposiciones Avanzadas del Mapa](#superposiciones-avanzadas-del-mapa)
   - [Soporte de Mods en Formato TOML](#soporte-de-mods-en-formato-toml)
   - [Opciones de Accesibilidad](#opciones-de-accesibilidad)

---

## 1. Instalación y Puesta en Marcha

### Requisitos del Sistema y Archivos

Para ejecutar `rrt2`, tu sistema debe contar con:
- **Sistema Operativo**: Linux (x86_64), Windows 10/11 (x86_64) o macOS 11+ (Apple Silicon o Intel x86_64).
- **Tarjeta Gráfica**: Compatible con Vulkan, DirectX 12 o Metal mediante la biblioteca `wgpu`.
- **Archivos del Juego Original**: Una copia legítima de *Railroad Tycoon II Platinum*. El motor requiere específicamente:
  - `data/RT2.PK2`: Paquete principal de sprites, terrenos e interfaz gráfica.
  - `RT2_PLAT.EXE`: Ejecutable original del cual se extraen estadísticas de vehículos y tablas de equilibrio.
  - `default2.lng`: Diccionario de textos originales del juego.
  - Directorios opcionales: `maps/` (mapas de escenarios), `games/` (partidas guardadas predefinidas) y `anim/` (secuencias cinemáticas `.SMK`; requiere tener instalados `ffmpeg` y `ffplay`).

### Detección Automática de Instalaciones

La primera vez que abres `rrt2`, se presenta la **Pantalla de Configuración Inicial**. El motor escanea tu equipo en un orden determinista para localizar instalaciones válidas:
- **Steam**: Carpetas de bibliotecas de Steam (`steamapps/common/Railroad Tycoon 2`), incluyendo Steam Flatpak y prefijos Proton (`compatdata/284840/pfx/drive_c`).
- **GOG**: Registros de Windows (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`), rutas típicas `GOG Games/Railroad Tycoon II` y directorios de Heroic Games Launcher.
- **Lutris y Wine**: Prefijos configurados en Lutris (`games/*.yml`), directorios estándar de Wine (`~/.wine`), Bottles y PlayOnLinux.
- **Carpetas Estándar de Windows**: `C:\Games\`, `C:\Program Files (x86)\`, explorando todas las unidades de `A:` a `Z:`.

Si se detectan instalaciones válidas, se muestran en una lista interactiva junto a su origen (por ejemplo, "Steam", "GOG", "Wine"). Selecciona la que desees y pulsa **Jugar**.

### Configuración Manual de la Ruta

Si tu juego se encuentra en una ubicación personalizada:
1. **Desde la Interfaz Gráfica**: Haz clic en **Elegir carpeta...** para utilizar el selector nativo de archivos del sistema (compatible con el portal XDG en Linux y Flatpak).
2. **Desde la Línea de Comandos**:
   ```bash
   ./rrt2 set-path ~/Juegos/railroad-tycoon-2
   # En Windows:
   rrt2.exe set-path "C:\Juegos\Railroad Tycoon II"
   ```
3. **Mediante Parámetro o Variable de Entorno**:
   ```bash
   ./rrt2 --game-dir /ruta/al/juego
   # O mediante variable de entorno:
   export RT2_DIR=/ruta/al/juego
   ./rrt2
   ```

### Paquetes por Sistema Operativo

- **Linux**:
  - **AppImage**: Ejecutable autónomo que integra fuentes vectoriales y traducciones. Concede permisos de ejecución (`chmod +x rrt2-*.AppImage`) y ejecútalo directamente.
  - **Flatpak**: Distribuido bajo el ID `io.github.pedryvo.rrt2.flatpak`. Opera aislado del sistema; emplea el selector de carpetas para otorgar acceso a los datos del juego.
  - **Tarball (`.tar.gz`)**: Descomprime y ejecuta `./rrt2`. Únicamente enlaza contra `glibc` y `libasound.so.2` (Vulkan y Wayland/X11 se cargan dinámicamente).
- **Windows**:
  - **Instalador Inno Setup (`-setup.exe`)**: No precisa privilegios de administrador, crea accesos directos en el Menú Inicio, incluye desinstalador en Configuración de Windows y memoriza la ruta del juego.
  - **winget**: Instalación automática desde la consola:
    ```cmd
    winget install pedryvo.rrt2
    ```
  - **ZIP Portable**: Extrae el archivo comprimido en cualquier directorio y ejecuta `rrt2.exe`.
- **macOS**:
  - **Imagen de Disco (`.dmg`)**: Abre el archivo `.dmg` y arrastra `RRT2.app` a la carpeta `/Applications`. Compatible con Apple Silicon (`aarch64`) e Intel (`x86_64`).
  - *Aviso de Gatekeeper*: Al tratarse de versiones no firmadas comercialmente, haz clic derecho sobre `RRT2.app` y pulsa **Abrir**, o ejecuta en el terminal:
    ```bash
    xattr -dr com.apple.quarantine /Applications/RRT2.app
    ```

### Directorios de Configuración y Partidas Guardadas

El motor nunca modifica ni escribe archivos en tu instalación original. Los datos del usuario se almacenan en ubicaciones dedicadas:

| Finalidad | Linux | Windows | macOS |
|---|---|---|---|
| Configuración (`config.toml`) | `~/.config/rrt2/` | `%APPDATA%\rrt2\` | `~/Library/Application Support/rrt2/` |
| Partidas Guardadas (`*.GAM`, `*.toml`, `*.png`) | `~/.local/share/rrt2/saves/` | `%APPDATA%\rrt2\saves\` | `~/Library/Application Support/rrt2/saves/` |
| Informes Exportados (`*.csv`) | `~/.local/share/rrt2/reports/` | `%APPDATA%\rrt2\reports\` | `~/Library/Application Support/rrt2/reports/` |
| Puntuaciones Máximas (`hiscore.dat`) | `~/.local/share/rrt2/` | `%APPDATA%\rrt2\` | `~/Library/Application Support/rrt2/` |

> [!TIP]
> Puedes cambiar estas rutas utilizando las variables de entorno `RRT2_CONFIG`, `RRT2_SAVES` y `RRT2_REPORTS`.

---

## 2. Primeros Pasos

### Modos de Juego: Campañas, Escenarios y Sandbox

Desde el menú **Un Jugador (Single Player)**, elige cómo forjar tu legado:
- **Campaña Clásica (Original)**: Supera los 18 escenarios históricos que narran la evolución del ferrocarril en Norteamérica y Europa (iniciando con pioneras como la *Baltimore & Ohio*). Cada mapa permite elegir una bonificación de partida (descuentos en locomotoras, subsidios de obra o menores tasas de interés).
- **Campaña The Second Century**: Explora los 18 escenarios avanzados de la expansión del siglo XX y XXI, con tracción eléctrica moderna, trenes expresos de pasajeros, transporte pesado y desafíos bélicos y de posguerra.
- **Escenarios Sueltos**: Elige cualquier mapa `.MAP` o `.MP2`. Consulta el informe de misión, revisa los objetivos de victoria (Bronce, Plata y Oro) y configura el nivel de dificultad económica e industrial.
- **Modo Sandbox**: Disfruta de la construcción sin presiones financieras, quiebras ni adquisiciones hostiles de los competidores.
- **Tutorial y Mapas de Ejemplo**: Muy recomendables para iniciarse. Carga el mapa de tutorial para aprender a tender vías y coordinar convoyes sin arriesgar tu capital.

### Fundar tu Compañía Ferroviaria

Cuando inicies un mapa sin una compañía asignada:
1. **Nombre e Imagen**: Escribe el nombre de tu empresa (hasta 30 caracteres) y selecciona el escudo y el color corporativo.
2. **Capital Inicial y Acciones**: Ajusta los selectores de constitución financiera. El panel te indicará:
   - Fondos iniciales en caja para operar.
   - Número de acciones emitidas y precio nominal de apertura.
   - Tu inversión personal como presidente ("You buy"): porcentaje accionarial que poseerás personalmente desde el primer día.
3. **Presidencia y Control**: ¡Mantén la rentabilidad! Si acumulas pérdidas continuas o vendes tus propias acciones, la junta de accionistas o un rival podrán promover tu destitución.

### Construcción de Vías Férreas

El tendido ferroviario es la clave de tu red:
1. Selecciona la herramienta **Lay Track** (Tender Vía) en la barra de herramientas izquierda.
2. Mantén presionado el **botón izquierdo del ratón** en la casilla de origen y arrastra el cursor por el mapa hacia el destino.
3. **Previsualización de Costes y Trazado en Tiempo Real**: Durante el arrastre, el sistema de guiado traza el recorrido óptimo, diseña puentes sobre cauces fluviales y detalla el coste total exacto. Si el trazado aparece en rojo, indica pendientes infranqueables o territorios sin concesión de paso.
4. Suelta el botón del ratón para autorizar el gasto y completar el tendido.
5. Pulsa `Esc` mientras arrastras para cancelar la acción sin coste.

> [!TIP]
> Mantén la inclinación de las vías por debajo del 2% siempre que sea posible. Pendientes de entre el 3% y el 6% reducen notablemente la velocidad de las locomotoras a vapor y multiplican el consumo de combustible.

### Estaciones y Mejoras de Infraestructura

Las estaciones captan viajeros, correo y mercancías en su entorno de influencia:
1. Haz clic en **Build Station** (Construir Estación).
2. **Tamaños de Estación**:
   - **Apeadero Pequeño (Small)**: Coste reducido ($50K), radio de captación de 2 casillas. Adecuado para industrias rurales aisladas.
   - **Estación Mediana (Medium)**: Coste intermedio ($100K), radio de 4 casillas. Idónea para núcleos urbanos en expansión.
   - **Terminal Grande (Large)**: Coste elevado ($200K), radio de 8 casillas. Imprescindible para grandes áreas metropolitanas.
3. **Círculo Dorado y Huella Visual**: Al situar el cursor sobre el terreno, un **círculo dorado** delimita el área de captación y el edificio proyectado muestra su viabilidad:
   - **Verde**: Ubicación apta y disponible.
   - **Amarillo**: Ubicación apta conectada a vías preexistentes.
   - **Rojo**: Ubicación no válida (obstáculos orográficos, agua o interferencia con otra estación).
4. **Estilos Arquitectónicos**: Elige entre estilo victoriano, colonial, misionero, tudor o kyoto para adaptar el aspecto visual a la región histórica.
5. **Mejoras de Estación**: Haz clic sobre una estación para abrir su menú de gestión e incorporar mejoras:
   - **Operativas**: *Torre de Agua* (Water Tower) y *Depósito de Arena* (Sandhouse) para el mantenimiento rápido de vapor; *Rotonda de Locomotoras* (Roundhouse), que recorta el mantenimiento de los motores en un 50%.
   - **Comerciales**: *Telégrafo* (acelera la recaudación), *Hotel* y *Restaurante* (incrementan los ingresos por pasajeros), *Aduana* (Customs House) para convoyes transfronterizos.
   - **Logísticas**: *Silo de Granos*, *Almacén Frigorífico* y *Cobertizo de Bienes* para resguardar excedentes y evitar pérdidas de flete.

### Compra de Locomotoras

Haz clic en cualquier estación o abre la lista general de trenes y selecciona **Purchase Train**:
- **Catálogo de Modelos**: Locomotoras disponibles según la época histórica, divididas en Vapor, Diésel y Eléctricas.
- **Matriz de Rendimiento**: Analiza la tabla de velocidad según la pendiente (0%, 2%, 4% y 6%) arrastrando entre 1 y 6 vagones de 30 a 50 toneladas.
- **Costes de Operación**: Examina la cuota de mantenimiento anual, el consumo de combustible, la fiabilidad mecánica y las curvas de aceleración antes de comprometer fondos.
- **Electrificación**: Ten en cuenta que las locomotoras eléctricas requieren que la línea férrea haya sido previamente electrificada.

### Montaje de Composiciones y Rutas

1. **Planificación de Paradas**: Abre el menú de gestión del tren y añade estaciones a la hoja de ruta mediante la lista o seleccionándolas en el minimapa.
2. **Asignación de Vagones**:
   - **Cargas Rápidas**: Pasajeros y Correo (gran margen de beneficio, sensibles al tiempo de entrega).
   - **Cargas Pesadas e Industriales**: Carbón, Mineral de Hierro, Trigo, Ganado, Madera, Petróleo, Acero, Bienes Manufacturados, Automóviles.
3. **Condiciones de Espera**: Configura si el tren debe partir de inmediato, esperar media carga o permanecer en el andén hasta completar el flete (`Wait for Full Cargo`).

### Gestión Financiera y Mercado Bursátil

Dominar los mercados es indispensable para triunfar:
- **Bonos Corporativos (Bonds)**: Emite bonos de deuda para sufragar grandes ampliaciones de vía. La tasa de interés depende directamente de tu calificación crediticia (desde AAA hasta bono basura F). Amortiza los bonos en cuanto dispongas de tesorería positiva.
- **Mercado Bursátil (`M`)**:
  - **Compra y Venta de Acciones**: Invierte fondos personales en tu propia compañía para afianzar el control, o compra acciones de empresas rivales para cobrar dividendos.
  - **Recompra de Acciones (Buyback)**: Utiliza la tesorería corporativa para recomprar acciones en el mercado abierto, elevando el beneficio por acción (BPA) y disparando la cotización.
  - **Desdoblamientos (Stock Splits)**: Si el precio por acción alcanza cotas elevadas, se produce un split 2 por 1 automático que devuelve liquidez al valor.
- **Dividendos**: Reparte dividendos anuales para recompensar a los accionistas y elevar el valor en bolsa, procurando no descapitalizar la empresa.
- **Fusiones y Adquisiciones**: Si adquieres más del 50% de las acciones de un competidor, podrás forzar una absorción para integrar sus vías, estaciones y trenes en tu red.

---

## 3. Controles y Atajos de Teclado

Todos los atajos se pueden personalizar en **Opciones (`F10`) > Teclas**.

### Navegación de Cámara y Zoom

| Acción | Tecla / Control | Notas |
|---|---|---|
| Desplazar Cámara | `W`, `A`, `S`, `D` o Flechas | Desplazamiento direccional fluido |
| Arrastre con Ratón | Mantener Botón Derecho y Arrastrar | Arrastre natural del terreno isométrico |
| Desplazamiento en Bordes | Cursor a menos de 4 px del margen | Configurable en Jugabilidad |
| Girar Cámara a la Izquierda | `Q` | Giro isométrico de 90° antihorario |
| Girar Cámara a la Derecha | `E` | Giro isométrico de 90° horario |
| Acercar (Zoom In) | Rueda del ratón arriba / `+` | Zoom suave y continuo |
| Alejar (Zoom Out) | Rueda del ratón abajo / `-` | Zoom suave y continuo |
| Centrar Cámara | `Home` | Devuelve la visión al punto de origen |

### Velocidad de Simulación y Pausa

| Acción | Atajo | Notas |
|---|---|---|
| Alternar Pausa | `Pause` o `Espacio` | Congela los cálculos de tiempo |
| Reducir Velocidad | `-` o `Numpad -` | Ralentiza el avance del calendario |
| Incrementar Velocidad | `+` o `Numpad +` | Acelera el avance del calendario |
| Pantalla Completa | `F11` | Alterna modo ventana y pantalla completa |
| Escala de Interfaz | `Ctrl +` / `Ctrl -` | Incrementos del 25% (entre 100% y 300%) |
| Silenciar Audio | `Ctrl + M` | Activa/desactiva música y efectos sonoros |
| Siguiente Pista Musical | `Ctrl + N` | Salta a la siguiente canción |

### Paneles de Información y Acceso Rápido

| Panel | Tecla | Información que Ofrece |
|---|---|---|
| **Finanzas** | `F` | Estado de resultados, balance patrimonial, deuda y crédito |
| **Bolsa de Valores** | `M` | Cotizaciones en directo, cartera de acciones y operaciones |
| **Lista de Trenes** | `T` | Resumen de todas las unidades, itinerarios, desgaste y edad |
| **Lista de Estaciones** | `G` | Volumen de fletes, mejoras instaladas, demanda local |
| **Ciudades** | `C` | Población urbana, ritmo demográfico y demandas de flete |
| **Industrias** | `I` | Rendimiento fabril, materias primas y adquisición de plantas |
| **Mapa General** | Barra izquierda (Icono del Globo) | Capas temáticas en vista cenital |
| **Guardado Rápido** | `F5` | Guarda en el espacio fijo quicksave |
| **Carga Rápida** | `F9` | Carga el último quicksave |
| **Menú Guardar** | `Ctrl + S` | Abre el gestor de partidas con nombre personalizado |
| **Menú Cargar** | `Ctrl + L` | Explorador de partidas con metadatos y capturas |
| **Opciones** | `F10` o `Esc` | Ajustes de pantalla, audio, controles y jugabilidad |

### Informes Financieros y Gráficos Interactivos

Accesibles desde el Libro de la Empresa (botón **Reports** o teclas `1`, `2`, `3`):
- **Gráficos Interactivos (Pestaña 1)**: Representa hasta 12 magnitudes históricas: Ingresos, Gastos, Beneficio Neto, Caja Disponible, Deuda Total, Patrimonio Neto, Precio de Acción, Valor Contable por Acción, Beneficio por Acción y Dividendos. Emplea la paleta accesible Okabe-Ito con formas geométricas, permitiendo activar y desactivar rivales e inspeccionar meses anteriores pasando el ratón.
- **Ingresos por Categoría (Pestaña 2)**: Desglose analítico de las 13 partidas de ingresos y gastos comparando el total histórico, el ejercicio corriente y los últimos 5 años fiscales.
- **Clasificaciones (Pestaña 3)**: Tablas ordenadas con los trenes más rentables y las estaciones de mayor tráfico. Un clic sobre cualquier entrada sitúa la cámara directamente sobre el tren o la terminal.
- **Exportación a CSV**: El botón **Export CSV** vuelca al instante los registros tabulares a `~/.local/share/rrt2/reports/*.csv` para su tratamiento en hojas de cálculo.

### Historial de Eventos y Notificaciones

Disponible en la Caja de Actividad (Activity Box) del HUD inferior:
- Pulsa **History** para consultar un registro de hasta 500 avisos (hitos de escenario, fluctuaciones económicas, movimientos bursátiles, crecimiento de ciudades y acciones de la competencia).
- Filtra por categorías temáticas o haz clic en **Show on Map** para trasladar la vista de forma inmediata a la casilla donde sucedió la noticia.
- Las alertas emergentes y noticias de periódico pueden habilitarse o desactivarse en **Opciones > Avisos**.

### Soporte para Mando y Steam Deck

Para partidas en Steam Deck o con mandos convencionales:
- **Stick Analógico Izquierdo**: Movimiento suave de la cámara por el escenario.
- **Stick Analógico Derecho**: Puntero virtual con asistencia de selección sobre vías e inmuebles.
- **Gatillos (L2 / R2)**: Ajuste directo de velocidad del tiempo y pausa rápida.
- **Botones Frontales (L1 / R1)**: Rotación de cámara (`Q` / `E`) y zoom.
- **Botonera Principal (A / B / X / Y)**: Aceptar, Cancelar / Cerrar, Herramienta de Vía, Herramienta de Estación.

---

## 4. Diferencias Respecto al Original de 1998

Conservando fielmente la identidad, el apartado visual y el encanto de *Railroad Tycoon II Platinum*, `rrt2-decomp` moderniza su arquitectura tecnológica:

```
┌─────────────────────────┬─────────────────────────────────┬─────────────────────────────────┐
│ Característica          │ Juego Original (1998–2001)       │ rrt2-decomp (Motor en Rust)     │
├─────────────────────────┼─────────────────────────────────┼─────────────────────────────────┤
│ Resolución de Pantalla  │ Fija en 800x600 o 1024x768      │ Nativa en 1080p, 1440p, 4K y UW │
│ Escala de la Interfaz   │ Sin escala (elementos diminutos)│ Tipografía vectorial (100%–300%)│
│ Precisión de Simulación │ Binario x86 propietario Win32   │ 100% determinista bit a bit     │
│ Renderizado Gráfico     │ DirectDraw / Software GDI       │ Aceleración por hardware (wgpu) │
│ Manejo de Cámara        │ Zoom por saltos, scroll tosco   │ Zoom continuo y arrastre fluido │
│ Protocolo Multijugador  │ DirectPlay 4, IPX, Módem, COM   │ Red lockstep moderna por TCP/IP │
│ Estabilidad de Red      │ Si cae el anfitrión, muere juego│ Migración de host y reconexión  │
│ Plataformas Compatibles │ Únicamente Windows 95/98/XP     │ Linux nativo, Windows y macOS   │
│ Idiomas Disponibles     │ Un único idioma por disco       │ 6 idiomas conmutables en juego  │
└─────────────────────────┴─────────────────────────────────┴─────────────────────────────────┘
```

### Simulación Bit a Bit Fiel y Determinista

El núcleo de simulación (`rt2-sim`) no recurre a aproximaciones arbitrarias. Todas las ecuaciones matemáticas —curvas de desaceleración de trenes, probabilidades de avería mecánica, generación de fletes urbanos, cotizaciones bursátiles y decisiones de la IA rival— han sido analizadas mediante ingeniería inversa del ejecutable original y contrastadas frente a emulación directa (Unicorn). Cualquier partida guardada cargada en el motor evoluciona de manera idéntica al ejecutable de 1998.

### Pantallas Modernas, Panorámicas y 4K

Se acabaron las deformaciones y el estiramiento de pantalla. El motor `wgpu` dibuja la malla del terreno, los ríos, los desniveles y las estaciones en la resolución real de tu monitor. Los sprites 2D originales reciben un procesado de reescalado de alta calidad (filtros xBRZ y algoritmos de interpolación inteligente) que se almacena en caché en la primera ejecución, garantizando nitidez cristalina en pantallas 4K y monitores ultrapanorámicos.

### Tipografía Vectorial Nítida y Soporte HiDPI

En lugar de fuentes de mapa de bits pixeladas, `rrt2-decomp` integra tipografías vectoriales abiertas bajo licencia SIL OFL, calibradas para reflejar la personalidad visual del juego:
- **Cinzel**: Versalitas con reminiscencia de grabado que sustituyen a Copperplate Gothic.
- **Libre Franklin**: Sans-serif precisa para tablas numéricas y botones.
- **Dancing Script**: Cursiva distinguida para cartas de felicitación y despachos de época.
- **Anton y Oswald**: Títulos densos de estilo gótico condensado.
- **Noto Sans SC**: Fuente tipográfica CJK integrada para la lectura fluida de caracteres chinos.
- **Cormorant SC**: Versalitas con cobertura cirílica completa para el idioma ruso.

La escala de la interfaz se adapta en pasos del 25% (del 100% al 300%), asegurando comodidad de lectura tanto en pantallas de televisión 4K como en consolas portátiles.

### Multijugador Lockstep Moderno por TCP/IP

La vieja infraestructura sustentada en DirectPlay 4, redes locales IPX y módems telefónicos ha sido reemplazada por una arquitectura de red directa y eficiente sobre TCP/IP:
- **Lockstep Secuenciado por el Anfitrión**: Las máquinas intercambian paquetes de comandos mínimos, manteniendo las simulaciones rigurosamente sincronizadas minuto a minuto.
- **Unión a Partidas en Curso y Reconexión**: Si sufres una desconexión accidental, puedes volver a unirte sin reiniciar la sesión; el anfitrión te enviará una captura comprimida del estado del mapa.
- **Migración Automática de Anfitrión**: Si el creador de la partida se desconecta, el jugador conectado con el identificador más bajo asume las labores de anfitrión de inmediato.
- **Traspaso Fluido a la IA**: Si un jugador abandona la partida, su compañía ferroviaria pasa a ser controlada por la inteligencia artificial para no romper el equilibrio de la sesión.

### Localización Dinámica en Seis Idiomas

Alterna de forma instantánea entre **English**, **Português**, **Español**, **Deutsch**, **中文 (Chino Simplificado)** y **Русский** desde **Opciones > Idioma**. La totalidad de textos, diálogos y mediciones tipográficas se actualizan al vuelo sin reiniciar la partida.

---

## 5. Características Modernas Opcionales

Todas las adiciones de comodidad son de carácter modular y opcional, preservando intacto el comportamiento clásico por defecto.

### Correcciones de Errores Originales por Regra (`Fix`)

El ejecutable original de 1998 albergaba ciertos desajustes documentados (anomalías de redondeo en acciones iniciales de fundadores, cobro duplicado en ciertas reformas de estación y desbordamientos de enteros en la tesorería tras varias décadas de juego). En `rrt2-decomp`, puedes elegir:
- **Modo Clásico Estricto (Predeterminado)**: Reproduce hasta el último detalle y peculiaridad histórica del juego de 1998 para garantizar la máxima fidelidad y compatibilidad con partidas antiguas.
- **Modo con Correcciones (`Fix`)**: Aplica soluciones matemáticas auditadas en los cálculos de balance, registrando la bandera correspondiente en la cabecera de la partida para preservar la reproducibilidad determinista.

### Deshacer Construcción (`Ctrl+Z`)

¿Trazaste por error un puente de gran coste o ubicaste una estación en la casilla equivocada? Pulsa `Ctrl + Z` para revertir al instante tu última acción de tendido de vías o colocación de edificios. Siempre que ningún convoy haya rodado por el segmento recién colocado, se te reintegrará el 100% del importe gastado.

### Gestor de Flota y Reemplazo Masivo

Gestionar decenas de trenes en extensas redes continentales ya no requiere interminables clics individuales:
- **Plantillas de Composición**: Guarda preajustes de flete y vagones (como "Expreso Nocturno de Pasajeros" o "Convoy Pesado de Carbón") y asígnalos a nuevas locomotoras con una sola pulsación.
- **Modernización Masiva de Locomotoras**: Selecciona una clase de motor desfasada en tu inventario y programa la sustitución automática para todos los convoyes conforme vayan recalando en estaciones provistas de rotonda de locomotoras.

### Superposiciones Avanzadas del Mapa

Más allá de los clásicos mapas de recursos e inclinación, benefíciate de nuevas capas analíticas:
- **Mapa de Calor de Oferta y Demanda**: Visualiza en segundos las regiones con exceso de producción y las ciudades con escasez apremiante de suministros.
- **Gradiente y Pendiente de Vías**: Mapa de colores que señala desniveles pronunciados para anticipar pérdidas de velocidad.
- **Rentabilidad por Ramal**: Identifica tramos ferroviarios altamente lucrativos y líneas deficitarias.
- **Densidad y Retenciones**: Localiza cruces críticos y trazados de vía única con embotellamientos y tiempos de espera desmedidos.

### Soporte de Mods en Formato TOML

Los datos de la simulación dejan de estar encerrados en estructuras binarias:
- Las especificaciones de trenes, tipos de vagón, esquemas industriales de producción y reglas de escenario pueden extraerse y cargarse desde archivos legibles en formato **TOML**.
- La comunidad de jugadores puede incorporar locomotoras históricas inéditas, inventar nuevas industrias y balancear escenarios personalizados sin necesidad de compilar código fuente.

### Opciones de Accesibilidad

- **Paletas Adaptadas para Daltonismo**: Los gráficos financieros y el minimapa de radar aplican la contrastada paleta Okabe-Ito, complementada con geometrías diferenciadas (círculos, rombos, triángulos, cruces) y tramas discontinuas de línea.
- **Tamaño de Letra Independiente**: Configura el tamaño del texto de lectura con independencia de la escala gráfica del HUD.
- **Subtítulos y Señales Visuales para Alertas**: Indicadores visuales en pantalla alertan a jugadores con dificultades auditivas ante averías graves de convoyes, sabotajes, asaltos a trenes o avisos de margen financiero.

---

*Railroad Tycoon es una marca registrada de sus respectivos titulares. Este proyecto libre y sin ánimo de lucro no guarda vinculación con Take-Two Interactive, PopTop Software ni Gathering of Developers.*
