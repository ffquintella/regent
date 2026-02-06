define openvox_stub::thing (
  String $name = $title,
  String $path = "/srv/openvox/${name}",
) {
  file { $path:
    ensure => directory,
  }
}
