class Atom < Formula
  desc "A lightning-fast, modal terminal editor written in Rust"
  homepage "https://github.com/gnuzd/atom"
  version "0.1.9"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/gnuzd/atom/releases/download/v0.1.9/atom-v0.1.9-aarch64-apple-darwin.tar.gz"
      sha256 "260168b8ec1228ffabdb4479950447d75c5192eebed493c33138d05e566ee623"
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
