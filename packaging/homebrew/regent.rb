# Regent Homebrew Formula
class Regent < Formula
  desc "High-performance Puppet Development Kit rebuild in Rust"
  homepage "https://github.com/seu-usuario/regent"
  version "0.1.1"
  
  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/seu-usuario/regent/releases/download/v0.1.1/regent-0.1.1-aarch64-apple-darwin.tar.gz"
      sha256 "" # Will be filled during release
    else
      url "https://github.com/seu-usuario/regent/releases/download/v0.1.1/regent-0.1.1-x86_64-apple-darwin.tar.gz"
      sha256 "" # Will be filled during release
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/seu-usuario/regent/releases/download/v0.1.1/regent-0.1.1-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "" # Will be filled during release
    else
      url "https://github.com/seu-usuario/regent/releases/download/v0.1.1/regent-0.1.1-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "" # Will be filled during release
    end
  end

  def install
    bin.install "regent"
    
    # Install man pages if available
    if (buildpath/"man").exist?
      man1.install Dir["man/*.1"]
    end
    
    # Install shell completions if available
    if (buildpath/"completions").exist?
      bash_completion.install "completions/regent.bash" => "regent"
      zsh_completion.install "completions/_regent"
      fish_completion.install "completions/regent.fish"
    end
  end

  test do
    system "#{bin}/regent", "--version"
    system "#{bin}/regent", "--help"
  end
end
