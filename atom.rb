class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.3.tar.gz"
  sha256 "4725bf65fca9159a13a23c2a9c52ad46f50c061fd68495b869a14b5ab2b49989"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/atom", "--version"
  end
end
