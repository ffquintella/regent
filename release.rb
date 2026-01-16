#!/usr/bin/env ruby
# Regent release script

require 'fileutils'
require 'json'
require 'open3'

class RegentReleaser
  def initialize(version)
    @version = version
  end

  def validate_version
    unless @version =~ /^\d+\.\d+\.\d+/
      puts "Invalid version format. Use semantic versioning (e.g., 1.2.3)"
      exit 1
    end
  end

  def update_rust_version
    cargo_toml = File.read('Cargo.toml')
    cargo_toml.gsub!(/version = "[^"]*"/, "version = \"#{@version}\"")
    File.write('Cargo.toml', cargo_toml)
    puts "✓ Updated Cargo.toml to version #{@version}"
  end

  def update_ruby_version
    version_rb = File.read('lib/regent/version.rb')
    version_rb.gsub!(/VERSION = "[^"]*"/, "VERSION = \"#{@version}\"")
    File.write('lib/regent/version.rb', version_rb)
    puts "✓ Updated version.rb to version #{@version}"
  end

  def build_and_test
    puts "Running tests..."
    stdout, stderr, status = Open3.capture3("cargo test")
    
    unless status.success?
      puts "Tests failed:"
      puts stderr
      exit 1
    end
    puts "✓ All tests passed"
  end

  def create_git_tag
    system("git add .")
    system("git commit -m \"Release version #{@version}\"")
    system("git tag -a v#{@version} -m \"Release version #{@version}\"")
    puts "✓ Created git tag v#{@version}"
  end

  def build_gem_and_binary
    system("ruby build.rb")
    puts "✓ Built gem and binary"
  end

  def run_release
    validate_version
    update_rust_version
    update_ruby_version
    build_and_test
    build_gem_and_binary
    create_git_tag
    
    puts "\n✓ Release v#{@version} complete!"
    puts "Next steps:"
    puts "  1. Push: git push origin main --tags"
    puts "  2. Publish gem: gem push regent-#{@version}.gem"
    puts "  3. Create GitHub release with the gem and binary"
  end
end

version = ARGV[0] || raise("Usage: ruby release.rb VERSION")
ReleaserRegent.new(version).run_release
