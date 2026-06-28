class Optimac < Formula
  desc "Safe macOS cleanup and maintenance CLI"
  homepage "https://github.com/agungdp150/optimac"
  url "https://github.com/agungdp150/optimac/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "af3e81ec284d66b6b62eefd1de01705fecfd1a6e80afcf3d779a828d07050273"
  license "MIT"

  depends_on "go" => :build

  def install
    system "go", "build", "-ldflags", "-X main.version=#{version}", "-o", bin/"optimac", "./cmd/optimac"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/optimac version")
  end
end
