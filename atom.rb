class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "20e4a378c2b98e3cbdcfc05660198f46d0c5b5a3d837bac6c147b356f8fd828a"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/atom", "--version"
  end
end
