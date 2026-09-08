# Homebrew formula for the geoparquet-validator command. Lives in the geoparquet/homebrew-tap
# repository as Formula/geoparquet-validator.rb; this copy is the source of truth and is updated
# with every release:  brew install geoparquet/tap/geoparquet-validator
class GeoparquetValidator < Formula
  desc "Validate GeoParquet files against the OGC abstract tests and the 1.0/1.1 community specifications"
  homepage "https://validator.geoparquet.org/"
  version "0.2.0"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.2.0/geoparquet-validator-v0.2.0-aarch64-apple-darwin.tar.gz"
      sha256 "6710267eb3ce06f0ad909ab1271bbf9e6a3ce6a726eaf84f4213bea30977565e"
    end
    on_intel do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.2.0/geoparquet-validator-v0.2.0-x86_64-apple-darwin.tar.gz"
      sha256 "29299396db661746b53fc7b5feba22c38a18cfd79d66126b9aef7046c5ddeecd"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.2.0/geoparquet-validator-v0.2.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "b2502e2e530f745f9bf256041cb46cde3e06bd7590f4fc282afb04c9847083f5"
    end
    on_intel do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.2.0/geoparquet-validator-v0.2.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "d16414b3e2afe5680b6a72118990653e7f5b020b9241fcf99a98f4c86cda92a0"
    end
  end

  def install
    bin.install "geoparquet-validator"
    lib.install Dir["libgeoparquet_validator.*"]
    include.install "include/geoparquet_validator.h"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/geoparquet-validator --version")
  end
end
