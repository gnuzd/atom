class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "6e96d6ccf7172d77a9a20a191fadd8da231b18625498a8d34f70991bb7ad7a48"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/atom", "--version"
  end
end
