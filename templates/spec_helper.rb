require 'rspec'
require 'puppet'
require 'puppet/util/log'

RSpec.configure do |config|
  config.mock_framework = :rspec
  config.mock_with :rspec do |c|
    c.syntax = :expect
  end
end

# Setup Puppet for testing
Puppet[:vardir] = '/tmp/puppet'
