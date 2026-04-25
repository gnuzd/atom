class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  version "0.1.12"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/gnuzd/atom/releases/download/v0.1.12/atom-v0.1.12-aarch64-apple-darwin.tar.gz"
      sha256 "f9351d3ef78a3ddb6cee29971bc996d9360c0cc88e3cde7daa7a4fcc17f05d33"
    else
      url "https://github.com/gnuzd/atom/releases/download/v0.1.12/atom-v0.1.12-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  on_linux do
    url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.12.tar.gz"
    sha256 "5d4c96de9cd0a3e4d3ccb02f15b3641167450ac1d5832b7de806dafbeb31f3d5"
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
