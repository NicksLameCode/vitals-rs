project('vitals',
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

cargo_target_dir = meson.project_build_root() / 'cargo-target'

cargo_build = custom_target('cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: ['vitals-build-stamp'],
  console: true,
  command: [
    'sh', '-c',
    cargo.full_path() + ' build' +
    ' --manifest-path ' + meson.project_source_root() / 'Cargo.toml' +
    ' --release' +
    ' --target-dir ' + cargo_target_dir +
    ' && touch @OUTPUT0@',
  ],
)

# Install binaries via meson.add_install_script so they are copied
# after the cargo build completes (install_data validates at configure time).
meson.add_install_script('sh', '-c',
  'install -Dm755 "' + cargo_target_dir / 'release' / 'vitals-app' + '" "$DESTDIR/@0@"'.format(bindir / 'vitals-app')
)
meson.add_install_script('sh', '-c',
  'install -Dm755 "' + cargo_target_dir / 'release' / 'vitals-daemon' + '" "$DESTDIR/@0@"'.format(bindir / 'vitals-daemon')
)

subdir('data')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  update_desktop_database: true,
)
