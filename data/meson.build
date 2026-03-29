desktop_file = 'com.corecoding.Vitals.desktop'
install_data(
  desktop_file,
  install_dir: get_option('datadir') / 'applications',
)

service_file = 'com.corecoding.Vitals.service'
install_data(
  service_file,
  install_dir: get_option('datadir') / 'dbus-1' / 'services',
)

install_data(
  'com.corecoding.Vitals.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

install_data(
  'com.corecoding.Vitals.metainfo.xml',
  install_dir: get_option('datadir') / 'metainfo',
)

icon_sizes = ['scalable']
icon_variants = ['gnome', 'original']

foreach variant : icon_variants
  install_subdir(
    'icons' / variant,
    install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps',
    strip_directory: true,
  )
endforeach
