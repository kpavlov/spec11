class Spec17Dev < Formula
  desc "Prebuilt development version of spec17 for Apple Silicon"
  homepage "https://spec17.dev"
  url "https://github.com/kpavlov/spec17/releases/download/edge/spec17-dev.20261120183742-darwin-aarch64.tar.gz"
  version "20261120183742"
  sha256 "770d3486bc71d517a253b4103725a30dac9434f70d995f705896b1f1b851c297"

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
