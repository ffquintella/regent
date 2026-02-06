class openvox_stub (
  Boolean $manage = true,
) {
  if !defined(Class['openvox_stub::params']) {
    class { 'openvox_stub::params': }
  }

  if $manage {
    file { $openvox_stub::params::data_dir:
      ensure => directory,
    }
  }
}
