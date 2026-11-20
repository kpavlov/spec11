class Spec17 < Formula
  desc "CLI, MCP server and web dashboard for spec-driven-development evidence"
  homepage "https://spec17.dev"

  # Begin stable release
  version "0.1.0"

  on_macos do
    on_arm do
      url "https://github.com/kpavlov/spec17/releases/download/v0.1.0/spec17-0.1.0-darwin-aarch64.tar.gz"
      sha256 "0202829b4b329674fbc79b8845a3b33bd7e5d79052e56525d8d3fd4df6a16cae"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/kpavlov/spec17/releases/download/v0.1.0/spec17-0.1.0-linux-aarch64.tar.gz"
      sha256 "167467cfcbe5508c0ca996be9275856bc278f8a737259fb2cf9fc25d5ac5ef6c"
    end
    on_intel do
      url "https://github.com/kpavlov/spec17/releases/download/v0.1.0/spec17-0.1.0-linux-x86_64.tar.gz"
      sha256 "e6039d4803fb75434616f8432cb8fcd91659322950a14c62ac36e3380a57737e"
    end
  end

  # End stable release

  conflicts_with "spec17-dev", because: "both install a spec17 binary"

  # (NFR-13) HEAD is available before the first tagged release.
  head do
    url "https://github.com/kpavlov/spec17.git", branch: "main"
    depends_on "node" => :build
    depends_on "pnpm" => :build
    depends_on "rust" => :build
  end

  def install
    if build.head?
      cd "web" do
        system "pnpm", "install", "--frozen-lockfile", "--store-dir", buildpath/".pnpm-store"
        system "pnpm", "run", "build"
      end
      ENV["SKIP_WEB_BUILD"] = "1"
      system "cargo", "install", *std_cargo_args(path: "server")
    else
      bin.install "spec17"
    end
  end

  test do
    system "#{bin}/spec17", "--version"
  end
end
