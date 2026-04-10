class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.4.tar.gz"
  sha256 "477d76512a3100a4c0e6c5e3dd9b4eac905aeac100d93b04576fd2eea422d1a6"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/atom", "--version"
  end
end
