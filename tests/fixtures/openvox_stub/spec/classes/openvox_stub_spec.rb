require 'spec_helper'

describe 'openvox_stub' do
  context 'with defaults' do
    it { is_expected.to compile }
    it { is_expected.to contain_class('openvox_stub::params') }
    it { is_expected.to contain_file('/srv/openvox/data') }
    it { is_expected.to contain_file('/srv/openvox/missing') }
  end
end
