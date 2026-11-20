class Spec17Dev < Formula
  desc "Prebuilt development version of spec17 for Apple Silicon"
  homepage "https://spec17.dev"
  url "https://github.com/kpavlov/spec17/releases/download/edge/spec17-dev.20261120230601-darwin-aarch64.tar.gz"
  version "20261120230601"
  sha256 "d5dd10115a28fb8b152af2bc385706699506c113cbd4da6a1d0f6fc9c04560bc"

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
