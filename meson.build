project('vitals',
  'rust',
  version: '1.0.0',
  license: 'BSD-3-Clause',
  meson_version: '>= 0.59.0',
)

gnome = import('gnome')
i18n = import('i18n')

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')

cargo = find_program('cargo')

cargo_build = custom_target('cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: ['vitals-app', 'vitals-daemon'],
  console: true,
  command: [
    cargo, 'build',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '--release',
    '--target-dir', meson.project_build_root() / 'cargo-target',
  ],
)

install_data(
  meson.project_build_root() / 'cargo-target' / 'release' / 'vitals-app',
  install_dir: bindir,
  install_mode: 'rwxr-xr-x',
)

install_data(
  meson.project_build_root() / 'cargo-target' / 'release' / 'vitals-daemon',
  install_dir: bindir,
  install_mode: 'rwxr-xr-x',
)

subdir('data')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  update_desktop_database: true,
)
