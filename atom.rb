class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  version "0.1.8"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/gnuzd/atom/releases/download/v0.1.8/atom-v0.1.8-aarch64-apple-darwin.tar.gz"
      sha256 "6236be0bd78d74029a876b30bca88a3bb50440eb148634ea9b88bfc18c948109"
    else
      url "https://github.com/gnuzd/atom/releases/download/v0.1.8/atom-v0.1.8-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  on_linux do
    url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.8.tar.gz"
    sha256 "77b4f21f7f39f1c90f2ba3ca56f3f3aef2b781140da115e289e4ae46636fb9df"
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
