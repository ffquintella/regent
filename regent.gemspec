# frozen_string_literal: true

require_relative 'lib/regent/version'

Gem::Specification.new do |spec|
  spec.name          = 'regent'
  spec.version       = Regent::VERSION
  spec.authors       = ['Felipe Quintella']
  spec.email         = ['ffquintella@gmail.com']

  spec.summary       = 'Regent - Rust + Artichoke Ruby Puppet Development Kit'
  spec.description   = 'A high-performance alternative development kit for Puppet modules built with Rust and Artichoke Ruby. Replaces the now paid PDK with better performance and full gem compatibility.'
  spec.homepage      = 'https://github.com/ffquintella/regent'
  spec.license       = 'AGPL-3.0'
  spec.required_ruby_version = '>= 2.6.0'

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = spec.homepage
  spec.metadata['changelog_uri'] = "#{spec.homepage}/blob/main/CHANGELOG.md"
  spec.metadata['documentation_uri'] = "#{spec.homepage}/wiki"
  spec.metadata['bug_tracker_uri'] = "#{spec.homepage}/issues"

  # Specify which files should be added to the gem when it is released.
  spec.files = Dir.glob([
    'lib/**/*',
    'exe/*',
    'LICENSE',
    'README.md',
    'ARCHITECTURE.md',
    'ARTICHOKE_INTEGRATION.md',
    'RUST_RUBY_INTEROP.md',
    'EXAMPLES.md',
    'CONTRIBUTING.md',
    'templates/**/*'
  ])
  
  spec.bindir        = 'exe'
  spec.executables   = spec.files.grep(%r{^exe/}) { |f| File.basename(f) }
  spec.require_paths = ['lib']

  # Runtime dependencies
  spec.add_dependency 'thor', '~> 1.2'
  spec.add_dependency 'tty-prompt', '~> 0.23'
  spec.add_dependency 'colorize', '~> 0.8'

  # Development dependencies
  spec.add_development_dependency 'rake', '~> 13.0'
  spec.add_development_dependency 'rspec', '~> 3.0'
  spec.add_development_dependency 'rubocop', '~> 1.21'
  spec.add_development_dependency 'yard', '~> 0.9'
end
