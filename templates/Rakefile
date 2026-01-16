# frozen_string_literal: true

require 'rspec/core/rake_task'

begin
  require 'rubocop/rake_task'
  RuboCop::RakeTask.new(:rubocop) do |task|
    task.patterns = ['lib/**/*.rb', 'spec/**/*.rb']
  end
rescue LoadError
  # rubocop is optional
end

task default: [:spec]

desc 'Run RSpec tests'
RSpec::Core::RakeTask.new(:spec) do |t|
  t.rspec_opts = '--color --format documentation'
  t.pattern = 'spec/**/*_spec.rb'
  t.fail_on_error = true
end

desc 'Run all tests'
task test: [:spec]
