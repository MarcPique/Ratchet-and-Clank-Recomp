<p align="center">
  <img src="assets/logo-transparente.png" alt="Ratchet & Clank Recomp" width="700">
</p>

<h1 align="center">Ratchet & Clank Recomp para PC</h1>

<p align="center">Código fuente y compilación personal para Windows.</p>

> **Créditos:** este proyecto está basado en [ReRAC: The Ratchet & Clank Rewrite](https://github.com/re-rac/rerac), desarrollado por los colaboradores de ReRAC. Su trabajo es la base del motor, la lógica del juego y la extracción de datos. Esta variante añade un instalador propio para Windows, opciones de interfaz y gráficos, y otros cambios descritos abajo. Se conserva su aviso de copyright y la licencia ISC en [LICENSE](LICENSE). Este repositorio no es una publicación oficial del equipo ReRAC.

## Instalar desde tu ISO

Descarga **Instalar-Ratchet-Clank.exe** desde [la última versión](https://github.com/MarcPique/Ratchet-and-Clank-Recomp/releases/latest). Elige tu ISO de *Ratchet & Clank* USA (SCUS_971.99) y una carpeta de destino nueva o vacía. El instalador verifica la ISO, extrae los datos, instala el juego y crea un acceso directo en el escritorio. La ventana del instalador usa el logo **Recomp**; el ejecutable del juego usa el logo original.

Al abrir el instalador puedes elegir **English, Français, Deutsch, Español o Italiano**. La interfaz cambia al instante y el juego arranca en ese idioma tras instalarse. Si ya existe un idioma guardado, el instalador lo muestra inicialmente. La instalación silenciosa acepta `--lang en|fr|de|es|it`.

La descarga **no contiene la ISO ni datos del juego**. Todo el contenido del juego se extrae localmente de tu propia ISO. Se necesitan aproximadamente 6 GB libres durante la instalación. El repositorio es privado para conservación personal.

El instalador es **un único EXE** con el logo y los archivos necesarios integrados. Su interfaz se adapta al tamaño de la ventana y a pantallas 4K con escala alta.

![Instalador renovado](screenshots/instalador-ventana.png)

![Selector de idioma del instalador](screenshots/instalador-idiomas.png)

[Ver el instalador maximizado a 4K](screenshots/instalador-4k.png).

## Mejoras incluidas

- Menú nativo de **Opciones avanzadas** en el menú de inicio y en las opciones durante la partida.
- Botón **Salir al escritorio** directamente en el menú principal y en el menú de pausa, con confirmación **¿Seguro que quieres salir?** antes del cierre normal de Windows.
- Desplazamiento del menú con mando o rueda del ratón; cuadro y tipografía adaptados a distintas resoluciones.
- Perfiles de imagen **Original**, **Mejorado** y **Personalizado**; cambios aplicados desde el propio juego.
- Resolución interna hasta 2160p, escala de renderizado, relación de aspecto, monitor, frecuencia de actualización y modo ventana/pantalla completa.
- Antialiasing MSAA, FXAA y SMAA; nitidez, filtrado anisotrópico, distancia de detalle, texturas, HUD y sombras.
- Sincronización vertical, límite de FPS y visualización de rendimiento.
- Control opcional de **DLSS 5** y perfiles de imagen desde Opciones avanzadas, sin abrir el panel de ReShade.
- Idioma del juego: inglés, francés, alemán, **español** e italiano.
- Iconos HD automáticos o manuales para DualSense, DualShock 3/4, Xbox 360/One/Series y Nintendo Switch.
- Sensibilidad de cámara, zona muerta, vibración de cámara, tamaño y contraste de subtítulos.
- Interfaz y texto preparados para 720p, 1080p y 4K; iconos corregidos para evitar duplicación visual.
- Ventana llamada **Ratchet & Clank**, icono original en la ventana, barra de tareas y ejecutable, y apertura sin terminal visible.

## Capturas reales

Capturas del juego a 3840 × 2160, tomadas en segundo plano. La [galería completa](screenshots/README.md) incluye las secciones principales de los ajustes gráficos y los nueve estilos de iconos de mando.

| Menú principal | Opciones avanzadas |
| --- | --- |
| ![Menú principal a 4K](screenshots/menu-principal-4k.png) | ![Opciones avanzadas a 4K](screenshots/graficos-perfil-4k.png) |

| Salir al escritorio desde el menú principal | Salir al escritorio desde pausa |
| --- | --- |
| ![Salir al escritorio en el menú principal a 4K](screenshots/salir-principal-4k.png) | ![Salir al escritorio en el menú de pausa a 4K](screenshots/salir-pausa-4k.png) |

| Confirmar salida desde el menú principal | Confirmar salida desde pausa |
| --- | --- |
| ![Confirmación de salida en el menú principal a 4K](screenshots/confirmar-salida-principal-4k.png) | ![Confirmación de salida en pausa a 4K](screenshots/confirmar-salida-pausa-4k.png) |

| Resolución y formato | Iconos DualSense |
| --- | --- |
| ![Resolución a 4K](screenshots/graficos-resolucion-4k.png) | ![Iconos DualSense a 4K](screenshots/iconos-dualsense-4k.png) |

### DLSS 5 opcional

El instalador personal ofrece **Instalar DLSS 5 (RTX)**. Incluye las tres bibliotecas de la configuración que ya funciona en una RTX 4090, verificadas por SHA-256, y descarga los demás componentes del [instalador de DLSS5-Feeder 1.19.0](https://github.com/jlrouzies-fr/DLSS5-Feeder/releases). La instalación requiere Internet y puede pedir permiso de administrador para registrar la capa Vulkan de ReShade. El juego arranca con el feeder preparado y permite encenderlo desde **Opciones avanzadas**. El panel interactivo de ReShade queda desactivado; su aviso inicial de carga aún puede aparecer unos segundos. **DLSS 5: Sí/No** cambia el `mode` del feeder entre 2 y 0 durante la partida, sin abrir el panel. El runtime se ha comprobado en una RTX 4090; no se ha verificado en cada modelo RTX.

La revisión del instalador 1.3.0 crea `dlss5-feed.cfg` durante la instalación. El instalador del feeder dejaba ese archivo para el primer arranque, mientras que el menú nativo necesita encontrarlo antes de iniciar el renderizador. Con esto desaparece el aviso de error al terminar la instalación y DLSS 5 puede activarse desde el juego.

Al activar DLSS 5, **Perfil DLSS 5** recorre **Cinemático → Calidad → Equilibrado → Rendimiento → Cinemático**. **Cinemático** da prioridad a la calidad: renderiza el juego al **100 %** de la resolución configurada y usa la pista temporal Transformer K. **Calidad** usa 75 % y Transformer J; **Equilibrado**, 67 % y la pista predeterminada; **Rendimiento**, 50 % y CNN E. El menú puede mostrar **Automático** o **Personalizado** al leer una configuración existente; al cambiar el perfil desde el juego se entra en el ciclo de cuatro modos. Los cambios se aplican durante la partida; la fila queda bloqueada cuando DLSS 5 está apagado o no está instalado.

Estos nombres describen perfiles de este port: las reducciones de resolución se hacen en el renderizador del juego antes de entregar la imagen al complemento. No son los modos oficiales DLSS Super Resolution, que el feeder no ofrece en Vulkan. Cinemático evita reducir la resolución interna, por lo que es la opción de mayor fidelidad y también la más exigente. Los ajustes manuales siguen disponibles en el menú.

El resultado depende del controlador y la GPU; el interruptor controla el feeder y no garantiza que el modelo neural acepte cada configuración. Para verificarlo, `ReShade.log` debe mostrar `state=ENGAGED` y evaluaciones correctas.

![DLSS 5 activado desde Opciones avanzadas a 4K](screenshots/dlss5-4k.png)

![Perfil DLSS 5 seleccionado desde Opciones avanzadas a 4K](screenshots/dlss5-perfiles-4k.png)


### Todos los estilos de iconos

![Comparativa de los nueve estilos de iconos HD](screenshots/iconos-hd-comparativa.png)

También hay capturas dentro del juego para [DualSense](screenshots/iconos-dualsense-4k.png), [Xbox Series](screenshots/iconos-xbox-series-4k.png) y [Nintendo Switch](screenshots/iconos-switch-4k.png). La [galería completa](screenshots/README.md) reúne las nueve familias.

## Código fuente

`crates/` contiene el motor, los menús, la extracción y las bibliotecas del juego. `installer/` contiene el instalador gráfico. `scripts/` construye el paquete Windows y el instalador. `assets/` contiene el logo Recomp del repositorio e instalador; el logo original del juego está en `crates/rc-engine/assets/`. `screenshots/` contiene las capturas de prueba.

Para recompilar en Windows:

1. Instala Rust y .NET 8 SDK.
2. Ejecuta `cargo build --release -p rc-engine -p rc-extract`.
3. Ejecuta `scripts/Build-GameBundle.ps1` con `-GameExe`, `-ExtractorExe`, `-ShadersDirectory`, `-LicenseFile` y `-OutputDirectory`.
4. Ejecuta `scripts/Build-Installer.ps1` con `-BundleZip`, `-OutputDirectory` y, para una compilación personal con DLSS 5, `-Dlss5SourceDirectory` apuntando a una instalación propia que ya funcione.

Los ZIP de compilación incrustados en el instalador se generan localmente y están excluidos de Git. El juego necesita los datos de tu ISO para iniciarse. Los avisos obligatorios de autoría y licencia están en [LICENSE](LICENSE); *Ratchet & Clank* pertenece a sus respectivos titulares.
