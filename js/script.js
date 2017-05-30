function hasHtml5Validation () {
  return typeof document.createElement('input').checkValidity === 'function';
}

$(function() {
    console.log("ready!");

    if (hasHtml5Validation()) {
      console.log("has html5 validation");
      $('#validate').submit(function (e) {
        console.log("submit");
        if (!this.checkValidity()) {
          e.preventDefault();
          console.log("invalid");
          $(this).addClass('invalid');
        } else {
          console.log("valid");
          $(this).addClass('valid');
        }

        if (!$('#pay_cash').is(":checked")) {
          alert("Please click the registration box to indicate you will pay cash for your registration when you arrive at the meeting.");
        }
      });
    }
});
