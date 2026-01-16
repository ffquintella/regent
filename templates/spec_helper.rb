# frozen_string_literal: true

require 'rspec'

RSpec.configure do |config|
  config.mock_framework = :rspec
  config.mock_with :rspec do |c|
    c.syntax = :expect
  end
  
  # Use color in output
  config.color = true
  
  # Format output
  config.formatter = :documentation
end
