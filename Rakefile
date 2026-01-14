# frozen_string_literal: true

require 'bundler/gem_tasks'
require 'rspec/core/rake_task'

RSpec::Core::RakeTask.new(:spec)

task default: :spec

desc 'Run tests'
task test: :spec

desc 'Install gem dependencies'
task :install do
  sh 'bundle install'
end

desc 'Show version'
task :version do
  require_relative 'lib/regent/version'
  puts "Regent version #{Regent::VERSION}"
end
