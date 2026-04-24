class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  version "0.1.11"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/gnuzd/atom/releases/download/v0.1.11/atom-v0.1.11-aarch64-apple-darwin.tar.gz"
      sha256 "c8bfc33f43404850349568e1e58335871483e2c2588c5015e31c623912489c16"
    else
      url "https://github.com/gnuzd/atom/releases/download/v0.1.11/atom-v0.1.11-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  on_linux do
    url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.11.tar.gz"
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
