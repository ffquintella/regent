require 'spec_helper'

describe 'openvox_stub::thing' do
  let(:title) { 'alpha' }

  it { is_expected.to compile }
  it { is_expected.to contain_file('/srv/openvox/alpha') }
  it { is_expected.to contain_file('/srv/openvox/alpha').with(ensure: 'absent') }
end
