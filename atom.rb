class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  version "0.1.13"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/gnuzd/atom/releases/download/v0.1.13/atom-v0.1.13-aarch64-apple-darwin.tar.gz"
      sha256 "b60f867ffc80960f05c92c80b2c6839447a319dc3c0808ebf3c0805ceea40507"
    else
      url "https://github.com/gnuzd/atom/releases/download/v0.1.13/atom-v0.1.13-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  on_linux do
    url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.13.tar.gz"
    sha256 "REPLACE_WITH_LINUX_SHA256"
    depends_on "rust" => :build
  end

  def install
    if OS.mac?
      bin.install "atom"
    else
      system "cargo", "install", *std_cargo_args
    end
  end

  test do
    system "#{bin}/atom", "--version"
  end
end
