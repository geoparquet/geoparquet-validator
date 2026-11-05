# Homebrew formula for the geoparquet-validator command. Lives in the geoparquet/homebrew-tap
# repository as Formula/geoparquet-validator.rb; this copy is the source of truth and is updated
# with every release:  brew install geoparquet/tap/geoparquet-validator
class GeoparquetValidator < Formula
  desc "Validate GeoParquet files against the spec and the OGC 2.0 abstract tests"
  homepage "https://validator.geoparquet.org/"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.3.1/geoparquet-validator-v0.3.1-aarch64-apple-darwin.tar.gz"
      sha256 "6b4df32496e8ed3839b5b4ff54b5238e36e981882d69413a7771d04df8535c0b"
    end
    on_intel do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.3.1/geoparquet-validator-v0.3.1-x86_64-apple-darwin.tar.gz"
      sha256 "8c1e12475a75e881749d7915729841718d554402843e4abd6d46ce95bbac0366"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.3.1/geoparquet-validator-v0.3.1-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "a01be9ee1ad584c8f1e503acd6e37e8f4f863495b50e5a71ceead12f6f604397"
    end
    on_intel do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.3.1/geoparquet-validator-v0.3.1-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "8fddd16f80140836d9cfd4ddb134a7f71fc942995def979ad7ad295821661cef"
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
