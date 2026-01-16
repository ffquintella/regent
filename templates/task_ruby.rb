#!/usr/bin/env ruby
# frozen_string_literal: true

# A Puppet task template written in Ruby
# Tasks are scripts that can be run on target systems via Puppet
# Learn more at: https://puppet.com/docs/bolt/latest/writing_tasks.html

require 'json'
require 'fileutils'

def run_task(params)
  {
    status: 'success',
    message: 'Task executed successfully',
    timestamp: Time.now.iso8601,
    params: params
  }
rescue StandardError => e
  {
    status: 'error',
    error_message: e.message,
    backtrace: e.backtrace
  }
end

# Parse input parameters from JSON (Bolt passes params via stdin)
params = JSON.parse($stdin.read) rescue {}

# Execute task and output result
result = run_task(params)
puts JSON.generate(result)
