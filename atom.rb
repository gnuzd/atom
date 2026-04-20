class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  version "0.1.7"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/gnuzd/atom/releases/download/v0.1.7/atom-v0.1.7-aarch64-apple-darwin.tar.gz"
      sha256 "5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8"
    else
      url "https://github.com/gnuzd/atom/releases/download/v0.1.7/atom-v0.1.7-x86_64-apple-darwin.tar.gz"
      sha256 "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
    end
  end

  on_linux do
    url "https://github.com/gnuzd/atom/archive/refs/tags/v0.1.7.tar.gz"
    sha256 "ac8b6fa6898b14d58cf91b55a8d0bb7e9d997b67f5609d74d74a429a45d84eb5"
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
