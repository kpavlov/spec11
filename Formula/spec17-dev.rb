class Spec17Dev < Formula
  desc "Prebuilt development version of spec17 for Apple Silicon"
  homepage "https://spec17.dev"
  url "https://github.com/kpavlov/spec17/releases/download/edge/spec17-dev.20261120201328-darwin-aarch64.tar.gz"
  version "20261120201328"
  sha256 "bf89b0addb9a5994adc3ceebaf000fde8e3060f470785387de83e9821e39b2fd"

  depends_on :macos
  depends_on arch: :arm64
  conflicts_with "spec17", because: "both install a spec17 binary"

  def install
    bin.install "spec17"
  end

  test do
    system "#{bin}/spec17", "--version"
  end
end
