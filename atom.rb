class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.5.tar.gz"
  sha256 "98f5bf4dae151de3aeb18970c8ec54984ff2483b814d13964c4625d92153b4b3"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/atom", "--version"
  end
end
