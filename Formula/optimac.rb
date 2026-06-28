class Optimac < Formula
  desc "Safe macOS cleanup and maintenance CLI"
  homepage "https://github.com/agungdp150/optimac"
  url "https://github.com/agungdp150/optimac/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "42789287818626a584a34565d7eecaa9dd1de7f24282f8cfbbd1ab7c14e18ff4"
  license "MIT"

  depends_on "go" => :build

  def install
    system "go", "build", "-ldflags", "-X main.version=#{version}", "-o", bin/"optimac", "./cmd/optimac"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/optimac version")
  end
end
