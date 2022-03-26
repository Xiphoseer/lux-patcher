# lux-patcher

A LU Patcher

## Local Installation

```console
$ git clone https://github.com/Xiphoseer/lux-patcher.git
$ cd lux-patcher
$ cargo install --path .
```

## Usage

```console
$ lux-patcher --cfg-url https://example.com/UniverseConfig/
```

## Options

`--env <environment>`
> Use the specified environment instead of `live` (e.g. `--env dev`)

`--install-dir <path>`
> Use the specified path for the installation instead of the one given in `patcher.ini` as `defaultinstallpath`

`--cfg-url <url>` (required)
> Use this URL to look up the universe configuration. Must end with a slash and host a valid `UniverseConfig.svc/xml/EnvironmentInfo` service.
