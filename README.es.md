[English](README.md) | [Español](README.es.md)

# rustyBolt

Un lanzador libre, portátil y multiplataforma para RuneLite y HDOS. Inicia sesión en tu cuenta de Jagex y arranca tu cliente.

## ¿Por qué lo hice?

El Bolt original es estupendo, pero cuesta un poco usarlo en macOS. El objetivo aquí es no depender del sistema operativo, tratar con cuidado los datos de la cuenta de Jagex y separar responsabilidades.

Juego a OSRS en un MacBook Pro, así que también hay un **modo wifi** que reduce la latencia de la red inalámbrica.

## Requisitos

- Un cliente de la lista aprobada, instalado y arrancado al menos una vez: [RuneLite](https://oldschool.runescape.wiki/w/RuneLite) o [HDOS](https://oldschool.runescape.wiki/w/HDOS). El Java que trae el instalador de RuneLite basta.
- Java 11 o más reciente.
- Un llavero del sistema para guardar el inicio de sesión de Jagex. macOS y Windows lo traen. En Linux hace falta un Secret Service como GNOME Keyring, KDE Wallet o KeePassXC; sin él el lanzador funciona igualmente, pero no puede mantener la sesión iniciada. Si GNOME pide una contraseña del llavero que tu contraseña de inicio de sesión no desbloquea, el llavero de inicio se creó con una contraseña antigua: cámbiala en Contraseñas y claves.

## Instalación

Descarga desde [Releases](https://github.com/nullparity/rustyBolt/releases).

| sistema | archivo | pasos |
| --- | --- | --- |
| macOS | `.tar.gz` | Descomprime. Arrastra `rustyBolt.app` a `/Applications`. |
| Windows | `.msi` | Doble clic. |
| Debian, Ubuntu | `.deb` | `sudo apt install ./rustybolt_*.deb` |
| Fedora, openSUSE | `.rpm` | `sudo dnf install ./rustybolt_*.rpm` (o `zypper`) |
| Otras distribuciones de Linux | `.AppImage` | `chmod +x` y doble clic. Necesita el `webkit2gtk-4.1` y el GTK 3 de la distribución (Fedora: `sudo dnf install webkit2gtk4.1`; para el icono de la bandeja, también `libayatana-appindicator-gtk3`). |

## Uso

1. Abre rustyBolt.
2. **Añadir cuenta de Jagex** e inicia sesión.
3. Elige un personaje, elige un cliente y pulsa **JUGAR**.

El idioma se cambia con el botón de la cabecera. Los ajustes, el modo wifi y la configuración de la JVM están en **Ajustes**. La línea de comandos está en [docs/cli.md](docs/cli.md) (en inglés).

## Documentación

La documentación detallada está en inglés:

- [Solución de problemas](docs/troubleshooting.md)
- [Línea de comandos](docs/cli.md)
- [Cómo funciona el inicio de sesión](docs/login.md)
- [Compilar desde el código fuente y contribuir](docs/building.md)
- [Arquitectura](docs/architecture.md)

## Aviso

rustyBolt es un proyecto no oficial. No está afiliado a Jagex, RuneLite ni HDOS. Esas partes no son responsables de ningún problema con rustyBolt ni de ningún daño que rustyBolt cause.

rustyBolt no es un cliente de juego. Ejecuta sin modificar los clientes que el usuario instaló. No puede modificar ni automatizar el juego. El lanzador usa solo los puntos de acceso públicos de inicio de sesión y nunca lee ni altera datos del juego.

RuneScape, Old School RuneScape y Jagex son marcas registradas de Jagex Limited.

## Licencia

MIT. Consulta [LICENSE](LICENSE).

Los archivos de cada versión llevan una atestación de procedencia de Sigstore. Para comprobar una descarga:

```
gh attestation verify rustybolt_<versión>_<plataforma>.tar.gz --repo nullparity/rustyBolt
```
