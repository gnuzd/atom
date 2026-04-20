class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  version "0.1.9"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/gnuzd/atom/releases/download/v0.1.9/atom-v0.1.9-aarch64-apple-darwin.tar.gz"
      sha256 "560bdb03f0fefe4708c5ff75c833d03d2814f9873d14cd5584b0922b5a6d52c7"
    else
      url "https://github.com/gnuzd/atom/releases/download/v0.1.9/atom-v0.1.9-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  on_linux do
    url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.9.tar.gz"
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
