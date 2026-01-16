#!/usr/bin/env ruby
# Regent build script for packaging the gem with compiled Rust binary

require 'fileutils'
require 'open3'

def build_rust_binary
  puts "Building Rust binary..."
  
  # Build Rust in release mode
  stdout, stderr, status = Open3.capture3("cargo build --release")
  
  unless status.success?
    puts "Rust build failed:"
    puts stderr
    exit 1
  end
  
  puts "✓ Rust binary built successfully"
end

def copy_binary_to_gem
  binary_src = "target/release/regent"
  binary_dest = "exe/regent"
  
  unless File.exist?(binary_src)
    puts "Error: Rust binary not found at #{binary_src}"
    exit 1
  end
  
  FileUtils.cp(binary_src, binary_dest)
  FileUtils.chmod(0o755, binary_dest)
  
  puts "✓ Binary copied to gem"
end

def build_gem
  puts "Building gem..."
  
  stdout, stderr, status = Open3.capture3("gem build regent.gemspec")
  
  unless status.success?
    puts "Gem build failed:"
    puts stderr
    exit 1
  end
  
  puts stdout
  puts "✓ Gem built successfully"
end

# Run the build process
build_rust_binary
copy_binary_to_gem
build_gem

puts "\n✓ Build complete!"
